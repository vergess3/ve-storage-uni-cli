use super::*;
use clap::Parser;
use std::io::{Read, Write};
use std::net::TcpListener;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Mutex;
use std::thread;

#[derive(Parser)]
struct CopyCommand {
    #[command(flatten)]
    arguments: CpArgs,
}

#[derive(Parser)]
struct SyncCommand {
    #[command(flatten)]
    arguments: SyncArgs,
}

struct TestDirectory(PathBuf);

impl TestDirectory {
    fn new() -> Self {
        let path =
            std::env::temp_dir().join(format!("adrive-directory-upload-{}", ulid::Ulid::new()));
        fs::create_dir_all(&path).unwrap();
        Self(path)
    }

    fn arguments(&self, flags: &[&str]) -> CpArgs {
        let mut arguments = vec![
            "cp",
            self.0.to_str().unwrap(),
            "adrive://inst/space/dst/",
            "--recursive",
            "--overwrite-strategy",
            "force",
        ];
        arguments.extend_from_slice(flags);
        CopyCommand::parse_from(arguments).arguments
    }
}

impl Drop for TestDirectory {
    fn drop(&mut self) {
        fs::remove_dir_all(&self.0).unwrap();
    }
}

#[derive(Default)]
struct FolderService {
    folders: BTreeSet<String>,
    files: BTreeSet<String>,
    requests: Vec<String>,
    denied_folder: Option<String>,
    conflict_folder: Option<(String, bool)>,
    is_paginated: bool,
    is_marker_invalid: bool,
    multipart_failure: Option<&'static str>,
    has_missing_failure_id: bool,
}

struct TestService {
    client: IdsClient,
    state: Arc<Mutex<FolderService>>,
    stop: Arc<AtomicBool>,
    worker: Option<thread::JoinHandle<()>>,
}

impl TestService {
    fn new() -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        listener.set_nonblocking(true).unwrap();
        let endpoint = format!("http://{}", listener.local_addr().unwrap());
        let state = Arc::new(Mutex::new(FolderService::default()));
        let stop = Arc::new(AtomicBool::new(false));
        let worker = Self::spawn(listener, state.clone(), stop.clone());
        let client = IdsClient::new(
            "fake-key".into(),
            "fake-secret".into(),
            None,
            Some(endpoint),
            Some("cn-beijing".into()),
            ClientOptions {
                max_retry_count: Some(0),
                requesttimeout: Some(2),
                ..Default::default()
            },
        )
        .unwrap();
        Self {
            client,
            state,
            stop,
            worker: Some(worker),
        }
    }

    fn spawn(
        listener: TcpListener,
        state: Arc<Mutex<FolderService>>,
        stop: Arc<AtomicBool>,
    ) -> thread::JoinHandle<()> {
        thread::spawn(move || {
            while !stop.load(Ordering::Relaxed) {
                match listener.accept() {
                    Ok((mut stream, _)) => {
                        // [Review Fix #DirectoryFixture] Accepted sockets can
                        // inherit the listener's nonblocking mode on macOS.
                        stream.set_nonblocking(false).unwrap();
                        stream
                            .set_read_timeout(Some(Duration::from_secs(2)))
                            .unwrap();
                        let request = read_service_request(&mut stream);
                        let (status, body) = service_response(&request, &mut state.lock().unwrap());
                        let request_id_header = {
                            let state = state.lock().unwrap();
                            if status == 403 && state.has_missing_failure_id {
                                String::new()
                            } else {
                                format!("x-request-id: service-{}\r\n", state.requests.len())
                            }
                        };
                        write!(stream, "HTTP/1.1 {status} Test\r\n{request_id_header}Content-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}", body.len()).unwrap();
                    }
                    Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                        thread::sleep(Duration::from_millis(2))
                    }
                    Err(error) => panic!("test server accept failed: {error}"),
                }
            }
        })
    }
}

impl Drop for TestService {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
        self.worker.take().unwrap().join().unwrap();
    }
}

fn read_service_request(stream: &mut std::net::TcpStream) -> String {
    let mut bytes = Vec::new();
    let mut buffer = [0_u8; 4096];
    loop {
        let count = stream.read(&mut buffer).unwrap();
        assert!(count > 0);
        bytes.extend_from_slice(&buffer[..count]);
        let Some(header_end) = bytes.windows(4).position(|part| part == b"\r\n\r\n") else {
            continue;
        };
        let content_length = String::from_utf8_lossy(&bytes[..header_end])
            .lines()
            .filter_map(|line| line.split_once(':'))
            .find(|(name, _)| name.eq_ignore_ascii_case("content-length"))
            .map(|(_, value)| value.trim().parse::<usize>().unwrap())
            .unwrap_or(0);
        if bytes.len() >= header_end + 4 + content_length {
            return String::from_utf8(bytes).unwrap();
        }
    }
}

fn service_response(request: &str, state: &mut FolderService) -> (u16, String) {
    let first = request.lines().next().unwrap();
    state.requests.push(request.to_string());
    if first.contains("initiateMultipart") {
        return (200, r#"{"UploadId":"upload-1"}"#.into());
    }
    if state
        .multipart_failure
        .is_some_and(|stage| first.contains(stage))
    {
        return (403, r#"{"Code":"Forbidden","Message":"denied"}"#.into());
    }
    if first.contains("uploadPart") {
        return (200, r#"{"PartNumber":1,"ETag":"part-etag"}"#.into());
    }
    if first.starts_with("GET ") {
        if state.is_marker_invalid {
            return (
                200,
                json!({"IsTruncated": true, "NextMarker": ""}).to_string(),
            );
        }
        if state.is_paginated && !first.contains("marker=next") {
            return (
                200,
                json!({"IsTruncated": true, "NextMarker": "next"}).to_string(),
            );
        }
        let folders = state
            .folders
            .iter()
            .map(|folder| json!({"Folder": folder}))
            .collect::<Vec<_>>();
        let files = state
            .files
            .iter()
            .map(|file| json!({"FilePath": file, "FileType": "file"}))
            .collect::<Vec<_>>();
        return (200, json!({"Files": files, "Folders": folders}).to_string());
    }
    if first.contains("/folders ") {
        let body: Value = serde_json::from_str(request.split("\r\n\r\n").nth(1).unwrap()).unwrap();
        let folder = body["FolderPath"].as_str().unwrap();
        if state.denied_folder.as_deref() == Some(folder) {
            return (403, r#"{"Code":"Forbidden","Message":"denied"}"#.into());
        }
        if let Some((conflict, has_racing_folder)) = &state.conflict_folder {
            if conflict == folder {
                if *has_racing_folder {
                    state.folders.insert(folder.to_string());
                }
                return (
                    409,
                    r#"{"Code":"Conflict","Message":"concurrent conflict"}"#.into(),
                );
            }
        }
        if state.files.contains(folder) {
            return (409, r#"{"Code":"Conflict","Message":"file exists"}"#.into());
        }
        state.folders.insert(folder.to_string());
        return (200, json!({"FolderPath": folder}).to_string());
    }
    (200, "{}".into())
}

#[tokio::test]
async fn directory_upload_preserves_empty_source_root() {
    let source = TestDirectory::new();
    let service = TestService::new();
    let arguments = source.arguments(&[]);
    let runtime = effective_cp_runtime_config(&Profile::default(), &arguments).unwrap();
    let report = upload_directory_recursive(&service.client, &arguments, runtime, false, false)
        .await
        .unwrap();
    assert_eq!(
        report.succeeded, 1,
        "an empty source must create the destination folder"
    );
    let manifest = report.manifest.unwrap();
    assert_eq!(manifest.object_count, 1);
    assert_eq!(manifest.total_size, 0);
    assert!(service.state.lock().unwrap().folders.contains("dst"));
}

#[test]
fn directory_upload_cli_uses_only_canonical_flag() {
    let flag = "--disable-dir-object";
    assert!(
        CopyCommand::try_parse_from([
            "cp",
            "local",
            "adrive://inst/space/dst",
            "--recursive",
            flag
        ])
        .unwrap()
        .arguments
        .disable_dir_object
    );
    assert!(
        SyncCommand::try_parse_from(["sync", "local", "adrive://inst/space/dst", flag])
            .unwrap()
            .arguments
            .disable_dir_object
    );
    assert!(CopyCommand::try_parse_from(["cp", "local", "adrive://inst/space/dst", flag]).is_err());
    for alias in ["--ddo", "-ddo"] {
        let cp_error = CopyCommand::try_parse_from([
            "cp",
            "local",
            "adrive://inst/space/dst",
            "--recursive",
            alias,
        ])
        .err()
        .expect("removed directory alias must fail");
        let sync_error =
            SyncCommand::try_parse_from(["sync", "local", "adrive://inst/space/dst", alias])
                .err()
                .expect("removed directory alias must fail");
        assert_eq!(cp_error.kind(), clap::error::ErrorKind::UnknownArgument);
        assert_eq!(sync_error.kind(), clap::error::ErrorKind::UnknownArgument);
    }
}

#[test]
fn directory_upload_describe_parameter_has_boolean_type() {
    assert_eq!(parameter_schema_type("disable-dir-object"), "boolean");
}

#[tokio::test]
async fn directory_upload_streaming_preserves_empty_nested_folders() {
    let source = TestDirectory::new();
    fs::create_dir_all(source.0.join("empty/deep")).unwrap();
    fs::write(source.0.join("zero.txt"), "").unwrap();
    let service = TestService::new();
    let arguments = source.arguments(&["--no-manifest"]);
    let runtime = effective_cp_runtime_config(&Profile::default(), &arguments).unwrap();
    let (report, scan, drain) = recursive_cp_streaming_no_manifest_report(
        &service.client,
        &arguments,
        runtime,
        "test",
        "test",
        false,
    )
    .await
    .unwrap();
    scan.unwrap();
    drain.unwrap();
    assert_eq!(report.succeeded, 4);
    assert_eq!(
        service.state.lock().unwrap().folders,
        BTreeSet::from(["dst".into(), "dst/empty".into(), "dst/empty/deep".into()])
    );
    assert!(report.manifest.is_none());
}

#[tokio::test]
async fn directory_upload_repeated_runs_use_typed_existing_folders() {
    let source = TestDirectory::new();
    fs::create_dir_all(source.0.join("empty")).unwrap();
    let service = TestService::new();
    let arguments = source.arguments(&[]);
    let runtime = effective_cp_runtime_config(&Profile::default(), &arguments).unwrap();
    for _ in 0..2 {
        let report = upload_directory_recursive(&service.client, &arguments, runtime, false, false)
            .await
            .unwrap();
        assert_eq!(report.failed, 0);
        assert_eq!(report.total, 2);
    }
    let state = service.state.lock().unwrap();
    assert_eq!(
        state
            .requests
            .iter()
            .filter(|request| request.starts_with("POST "))
            .count(),
        2
    );
}

#[tokio::test]
async fn directory_upload_reports_file_conflicts_and_forbidden_errors() {
    for is_file_conflict in [true, false] {
        let source = TestDirectory::new();
        let service = TestService::new();
        if is_file_conflict {
            service.state.lock().unwrap().files.insert("dst".into());
        } else {
            service.state.lock().unwrap().denied_folder = Some("dst".into());
        }
        let arguments = source.arguments(&[]);
        let runtime = effective_cp_runtime_config(&Profile::default(), &arguments).unwrap();
        let report = upload_directory_recursive(&service.client, &arguments, runtime, false, false)
            .await
            .unwrap();
        assert_eq!(report.failed, 1);
        assert_eq!(report.succeeded, 0);
    }
}

#[test]
fn directory_upload_empty_include_parent_root_is_desired_by_sync() {
    let source = TestDirectory::new();
    let desired =
        collect_local_sync_desired_relative_paths(&source.0, Some("src"), None, None).unwrap();
    assert!(desired.folders.contains("src"));
}

#[tokio::test]
async fn directory_upload_dry_run_describes_directory_policy_without_mutation() {
    let source = TestDirectory::new();
    let config = source.0.join("config.toml");
    fs::write(&config, "").unwrap();
    let global = GlobalArgs {
        config_path: Some(config),
        ..Default::default()
    };
    let auth = ADriveAuthArgs {
        auth_mode: Some(AuthMode::Aksk),
    };
    for is_disabled in [false, true] {
        let arguments = source.arguments(if is_disabled {
            &["--disable-dir-object"]
        } else {
            &[]
        });
        let plan = build_plan(&global, &auth, &ADriveCommand::Cp(arguments))
            .await
            .unwrap();
        assert_eq!(plan["directory_upload"]["enabled"], !is_disabled);
        assert_eq!(plan["directory_upload"]["implicit_file_parents"], true);
        assert_eq!(
            plan["request_plan"]
                .as_array()
                .unwrap()
                .iter()
                .any(|action| action == "create_folder"),
            !is_disabled
        );
    }
    assert_eq!(fs::read_dir(&source.0).unwrap().count(), 1);
}

#[tokio::test]
async fn directory_upload_dry_run_rejects_disable_for_remote_sources() {
    let source = TestDirectory::new();
    let config = source.0.join("config.toml");
    fs::write(&config, "").unwrap();
    let global = GlobalArgs {
        config_path: Some(config),
        ..Default::default()
    };
    let auth = ADriveAuthArgs {
        auth_mode: Some(AuthMode::Aksk),
    };
    let mut arguments = source.arguments(&["--disable-dir-object"]);
    arguments.source = "adrive://inst/space/src/".into();
    arguments.destination = source.0.display().to_string();
    assert!(build_plan(&global, &auth, &ADriveCommand::Cp(arguments))
        .await
        .is_err());
}

#[tokio::test]
async fn directory_upload_disable_skips_creation_in_both_manifest_modes() {
    let source = TestDirectory::new();
    fs::create_dir_all(source.0.join("empty")).unwrap();
    fs::write(source.0.join("zero.txt"), "").unwrap();
    let service = TestService::new();
    let arguments = source.arguments(&["--disable-dir-object"]);
    let runtime = effective_cp_runtime_config(&Profile::default(), &arguments).unwrap();
    let report = upload_directory_recursive(&service.client, &arguments, runtime, false, false)
        .await
        .unwrap();
    assert_eq!(report.total, 1);
    let (report, scan, drain) = recursive_cp_streaming_no_manifest_report(
        &service.client,
        &arguments,
        runtime,
        "test",
        "test",
        false,
    )
    .await
    .unwrap();
    scan.unwrap();
    drain.unwrap();
    assert_eq!(report.total, 1);
    let state = service.state.lock().unwrap();
    assert!(state.folders.is_empty());
    assert!(state.requests.iter().all(|request| !request
        .lines()
        .next()
        .unwrap()
        .contains("/folders")));
}

#[tokio::test]
async fn directory_upload_conflict_requires_exact_typed_folder() {
    for has_racing_folder in [false, true] {
        let source = TestDirectory::new();
        let service = TestService::new();
        service.state.lock().unwrap().conflict_folder = Some(("dst".into(), has_racing_folder));
        let arguments = source.arguments(&[]);
        let runtime = effective_cp_runtime_config(&Profile::default(), &arguments).unwrap();
        let report = upload_directory_recursive(&service.client, &arguments, runtime, false, false)
            .await
            .unwrap();
        assert_eq!(report.failed == 0, has_racing_folder);
    }
}

#[tokio::test]
async fn directory_upload_checkpoint_revalidates_missing_folders() {
    let source = TestDirectory::new();
    let checkpoint = TestDirectory::new();
    let service = TestService::new();
    let arguments = source.arguments(&[
        "--checkpoint",
        "--checkpoint-dir",
        checkpoint.0.to_str().unwrap(),
    ]);
    let runtime = effective_cp_runtime_config(&Profile::default(), &arguments).unwrap();
    for _ in 0..2 {
        let report = upload_directory_recursive(&service.client, &arguments, runtime, false, false)
            .await
            .unwrap();
        assert_eq!(report.succeeded, 1);
        assert_eq!(service.state.lock().unwrap().folders.len(), 1);
        service.state.lock().unwrap().folders.clear();
    }
    assert_eq!(
        service
            .state
            .lock()
            .unwrap()
            .requests
            .iter()
            .filter(|request| request.starts_with("POST "))
            .count(),
        2
    );
}

#[tokio::test]
async fn directory_upload_filters_keep_file_ancestors_without_extra_empty_folders() {
    let source = TestDirectory::new();
    fs::create_dir_all(source.0.join("parent/deep")).unwrap();
    fs::create_dir_all(source.0.join("unselected")).unwrap();
    fs::write(source.0.join("parent/deep/keep.txt"), "payload").unwrap();
    let service = TestService::new();
    let arguments = source.arguments(&["--include", "*.txt"]);
    let runtime = effective_cp_runtime_config(&Profile::default(), &arguments).unwrap();
    let report = upload_directory_recursive(&service.client, &arguments, runtime, false, false)
        .await
        .unwrap();
    assert_eq!(report.total, 4);
    let (report, scan, drain) = recursive_cp_streaming_no_manifest_report(
        &service.client,
        &arguments,
        runtime,
        "test",
        "test",
        false,
    )
    .await
    .unwrap();
    scan.unwrap();
    drain.unwrap();
    assert_eq!(report.total, 4);
    assert_eq!(
        service.state.lock().unwrap().folders,
        BTreeSet::from(["dst".into(), "dst/parent".into(), "dst/parent/deep".into()])
    );
}

#[tokio::test]
async fn directory_upload_preserves_include_parent_and_skips_space_root() {
    let source = TestDirectory::new();
    let service = TestService::new();
    let mut arguments = source.arguments(&["--include-parent"]);
    let runtime = effective_cp_runtime_config(&Profile::default(), &arguments).unwrap();
    let report = upload_directory_recursive(&service.client, &arguments, runtime, false, false)
        .await
        .unwrap();
    assert_eq!(report.total, 1);
    let parent = source.0.file_name().unwrap().to_str().unwrap();
    assert!(service
        .state
        .lock()
        .unwrap()
        .folders
        .contains(&format!("dst/{parent}")));
    arguments.include_parent = false;
    arguments.destination = "adrive://inst/space".into();
    let report = upload_directory_recursive(&service.client, &arguments, runtime, false, false)
        .await
        .unwrap();
    assert_eq!(report.total, 0);
}

#[cfg(unix)]
#[tokio::test]
async fn directory_upload_does_not_follow_symlink_descendants() {
    let source = TestDirectory::new();
    std::os::unix::fs::symlink(&source.0, source.0.join("loop")).unwrap();
    let service = TestService::new();
    let arguments = source.arguments(&[]);
    let runtime = effective_cp_runtime_config(&Profile::default(), &arguments).unwrap();
    let (report, scan, drain) = recursive_cp_streaming_no_manifest_report(
        &service.client,
        &arguments,
        runtime,
        "test",
        "test",
        false,
    )
    .await
    .unwrap();
    scan.unwrap();
    drain.unwrap();
    assert_eq!(report.total, 1);
    let report = upload_directory_recursive(&service.client, &arguments, runtime, false, false)
        .await
        .unwrap();
    assert_eq!(report.total, 1);
}

fn sync_arguments(source: &TestDirectory, checkpoint: &TestDirectory, flags: &[&str]) -> SyncArgs {
    let report = checkpoint.0.join("report.csv");
    let mut arguments = vec![
        "sync",
        source.0.to_str().unwrap(),
        "adrive://inst/space/dst/",
        "--delete",
        "--force",
        "--checkpoint-dir",
        checkpoint.0.to_str().unwrap(),
        "--report-path",
        report.to_str().unwrap(),
    ];
    arguments.extend_from_slice(flags);
    SyncCommand::parse_from(arguments).arguments
}

#[tokio::test]
async fn directory_upload_sync_creation_failure_blocks_deletion() {
    let source = TestDirectory::new();
    let checkpoint = TestDirectory::new();
    let service = TestService::new();
    {
        let mut state = service.state.lock().unwrap();
        state.denied_folder = Some("dst".into());
        state.folders.insert("dst/stale".into());
    }
    for flags in [&[][..], &["--no-manifest"][..]] {
        let arguments = sync_arguments(&source, &checkpoint, flags);
        let exit_code = execute_sync(
            &GlobalArgs::default(),
            &Profile::default(),
            &service.client,
            &arguments,
        )
        .await
        .unwrap();
        assert_eq!(exit_code, 1);
    }
    let state = service.state.lock().unwrap();
    assert!(state
        .requests
        .iter()
        .all(|request| !request.starts_with("DELETE ")));
}

#[tokio::test]
async fn directory_upload_sync_disable_protects_source_present_empty_folders() {
    let source = TestDirectory::new();
    fs::create_dir_all(source.0.join("keep")).unwrap();
    let checkpoint = TestDirectory::new();
    let service = TestService::new();
    service
        .state
        .lock()
        .unwrap()
        .folders
        .extend(["dst/keep".into(), "dst/stale".into()]);
    let arguments = sync_arguments(&source, &checkpoint, &["--disable-dir-object"]);
    let plan = build_adrive_sync_delete_plan(&service.client, &arguments, 2)
        .await
        .unwrap();
    assert_eq!(plan.len(), 1);
    assert_eq!(plan[0].source, "adrive://inst/space/dst/stale");
}

#[tokio::test]
async fn directory_upload_existing_folder_lookup_handles_pagination() {
    for is_marker_invalid in [false, true] {
        let source = TestDirectory::new();
        let service = TestService::new();
        {
            let mut state = service.state.lock().unwrap();
            state.folders.insert("dst".into());
            state.is_paginated = true;
            state.is_marker_invalid = is_marker_invalid;
        }
        let arguments = source.arguments(&[]);
        let runtime = effective_cp_runtime_config(&Profile::default(), &arguments).unwrap();
        let report = upload_directory_recursive(&service.client, &arguments, runtime, false, false)
            .await
            .unwrap();
        assert_eq!(report.failed == 1, is_marker_invalid);
        assert_eq!(
            service
                .state
                .lock()
                .unwrap()
                .requests
                .iter()
                .filter(|request| request.starts_with("POST "))
                .count(),
            0
        );
    }
}

#[tokio::test]
async fn report_request_id_matches_each_upload_after_command_limit() {
    for is_streaming in [false, true] {
        let source = TestDirectory::new();
        for index in 0..105 {
            fs::write(source.0.join(format!("file-{index:03}.txt")), "").unwrap();
        }
        let service = TestService::new();
        let trace = Arc::new(tos_core::agent::request_id::ServiceRequestTrace::default());
        let client = service.client.clone().with_request_trace(trace.clone());
        let flags: &[&str] = if is_streaming {
            &["--no-manifest"]
        } else {
            &[]
        };
        let arguments = source.arguments(flags);
        let runtime = effective_cp_runtime_config(&Profile::default(), &arguments).unwrap();
        let report = if is_streaming {
            let (report, scan, drain) = recursive_cp_streaming_no_manifest_report(
                &client, &arguments, runtime, "test", "test", false,
            )
            .await
            .unwrap();
            scan.unwrap();
            drain.unwrap();
            report
        } else {
            upload_directory_recursive(&client, &arguments, runtime, false, false)
                .await
                .unwrap()
        };
        assert_eq!(report.succeeded, 106);
        assert_eq!(trace.snapshot().request_ids.len(), 100);
        assert!(trace.snapshot().request_ids_omitted > 0);
        assert_report_upload_request_ids(&report, &service.state.lock().unwrap().requests);
    }
}

fn assert_report_upload_request_ids(report: &BatchReport, requests: &[String]) {
    for item in &report.items {
        let filename = Path::new(&item.source)
            .file_name()
            .unwrap()
            .to_str()
            .unwrap();
        let request_index = requests
            .iter()
            .position(|request| {
                if item.operation == "create-folder" {
                    request.starts_with("POST ") && request.contains("\"FolderPath\":\"dst\"")
                } else {
                    request.starts_with("POST ")
                        && request.lines().next().unwrap().contains(filename)
                }
            })
            .unwrap_or_else(|| {
                panic!(
                    "no request for {} ({}) among {:?}",
                    item.source,
                    item.operation,
                    requests
                        .iter()
                        .take(5)
                        .map(|request| request.lines().next())
                        .collect::<Vec<_>>()
                )
            });
        assert_eq!(
            item.request_id,
            Some(format!("service-{}", request_index + 1)),
            "{}",
            item.source
        );
    }
}

#[tokio::test]
async fn report_request_id_is_preserved_for_directory_denial() {
    let source = TestDirectory::new();
    let service = TestService::new();
    service.state.lock().unwrap().denied_folder = Some("dst".into());
    let arguments = source.arguments(&[]);
    let runtime = effective_cp_runtime_config(&Profile::default(), &arguments).unwrap();
    let report = upload_directory_recursive(&service.client, &arguments, runtime, false, false)
        .await
        .unwrap();
    assert_eq!(report.failed, 1);
    assert_eq!(report.items[0].request_id.as_deref(), Some("service-2"));
}

#[tokio::test]
async fn report_request_id_is_preserved_for_sync_delete_rows() {
    let service = TestService::new();
    let mut report = BatchReport::new("ve-adrive sync");
    let items = [SYNC_DELETE_EXTRA_FILE, SYNC_DELETE_EXTRA_FOLDER]
        .into_iter()
        .map(|operation| BatchManifestItem {
            operation: operation.into(),
            source: format!("adrive://inst/space/{operation}"),
            destination: None,
            size: 0,
            etag: None,
            crc64: None,
        })
        .collect();
    let deleted = execute_adrive_sync_delete_plan(&service.client, items, 2, false, &mut report)
        .await
        .unwrap();
    assert_eq!(deleted, 2);
    assert_eq!(report.items[0].request_id.as_deref(), Some("service-1"));
    assert_eq!(report.items[1].request_id.as_deref(), Some("service-2"));
}

#[tokio::test]
async fn report_request_id_preserves_multipart_failure_before_abort_cleanup() {
    for stage in ["uploadPart", "completeMultipart"] {
        for has_missing_failure_id in [false, true] {
            let (report, requests) = failed_multipart_report(stage, has_missing_failure_id).await;
            assert_eq!(report.failed, 1);
            assert!(
                requests.last().unwrap().contains("abortMultipart"),
                "report: {:?}, requests: {:?}",
                report.items,
                requests
                    .iter()
                    .map(|request| request.lines().next())
                    .collect::<Vec<_>>()
            );
            let failed_index = requests
                .iter()
                .position(|request| request.lines().next().unwrap().contains(stage))
                .unwrap();
            let expected =
                (!has_missing_failure_id).then(|| format!("service-{}", failed_index + 1));
            assert_eq!(
                report.items[0].request_id, expected,
                "{stage}, missing ID: {has_missing_failure_id}"
            );
        }
    }
}

async fn failed_multipart_report(
    stage: &'static str,
    has_missing_failure_id: bool,
) -> (BatchReport, Vec<String>) {
    let source = TestDirectory::new();
    let source_file = source.0.join("file.txt");
    fs::write(&source_file, "x").unwrap();
    let service = TestService::new();
    {
        let mut state = service.state.lock().unwrap();
        state.multipart_failure = Some(stage);
        state.has_missing_failure_id = has_missing_failure_id;
    }
    let arguments = source.arguments(&[]);
    let runtime = effective_cp_runtime_config(&Profile::default(), &arguments).unwrap();
    let outcome = capture_service_request_id(upload_adrive_file_multipart_for_batch(
        &service.client,
        "inst",
        "space",
        "file.txt",
        &source_file,
        1,
        false,
        None,
        None,
        runtime,
    ))
    .await;
    let mut report = BatchReport::new("ve-adrive cp");
    report.push_operation_result(source_file.display().to_string(), None, "upload", outcome);
    let requests = service.state.lock().unwrap().requests.clone();
    (report, requests)
}
