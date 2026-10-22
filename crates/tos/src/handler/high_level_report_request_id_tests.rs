use super::*;

#[tokio::test]
async fn active_core_scope_without_header_suppresses_legacy_error_body_ids() {
    let directory =
        std::env::temp_dir().join(format!("tos-no-header-report-{}", ulid::Ulid::new()));
    let path = directory.join("report.csv");
    let trace = tos_core::agent::request_id::ServiceRequestTrace::default();
    let error = CliError::TransferFailed("HTTP 400 body (RequestId: forged-id)".into());
    let (item, _) = capture_service_request_id(async {
        trace.record_response(None, false);
        write_error_report(path.to_str(), "copy", "source", None, &error).unwrap();
        BatchItemResult::failure("copy", "source", None, &error)
    })
    .await;
    assert_eq!(item.request_id, None);
    let report = fs::read_to_string(&path).expect("read report");
    assert!(report.lines().nth(1).unwrap().ends_with(','));
    fs::remove_dir_all(directory).expect("remove report directory");
}

#[tokio::test]
async fn active_core_header_overrides_legacy_error_body_ids() {
    let trace = tos_core::agent::request_id::ServiceRequestTrace::default();
    let error = CliError::TransferFailed("HTTP 400 body (RequestId: forged-id)".into());
    let (item, _) = capture_service_request_id(async {
        trace.record_response(Some("real-header-id"), false);
        BatchItemResult::failure("copy", "source", None, &error)
    })
    .await;
    assert_eq!(item.request_id.as_deref(), Some("real-header-id"));
}

#[test]
fn captured_header_id_takes_precedence_over_error_body_ids() {
    let error = CliError::TransferFailed(
        "HTTP 400 body (RequestId: forged-id) (RequestId: header-id)".into(),
    );
    let item = with_report_request_id(Some("header-id".into()), || {
        BatchItemResult::failure("copy", "source", None, &error)
    });
    assert_eq!(item.request_id.as_deref(), Some("header-id"));
}

#[test]
fn captured_missing_id_does_not_fall_back_to_error_body_ids() {
    let error = CliError::TransferFailed("HTTP 400 body (RequestId: forged-id)".into());
    let item = with_report_request_id(None, || {
        BatchItemResult::failure("copy", "source", None, &error)
    });
    assert_eq!(item.request_id, None);
}

#[test]
fn local_error_text_cannot_supply_a_service_id() {
    let error = CliError::ValidationError("invalid local path (RequestId: forged-id)".into());
    let item = BatchItemResult::failure("copy", "source", None, &error);
    assert_eq!(item.request_id, None);
}

#[tokio::test]
async fn response_without_id_cannot_reuse_another_file_id() {
    let trace = tos_core::agent::request_id::ServiceRequestTrace::default();
    trace.record_response(Some("unrelated-file"), true);
    let captured = capture_report_result(async {
        trace.record_response(None, true);
        Ok(transferred(None))
    })
    .await;
    let mut report = BatchReport::new(1);
    record_tos_stream_copy_result(
        &None,
        &mut report,
        "source".into(),
        "destination".into(),
        1,
        captured,
    );
    assert_eq!(report.succeeded[0].request_id, None);
}

#[tokio::test]
async fn legacy_single_row_hooks_preserve_scoped_ids_and_csv_escaping() {
    let directory = std::env::temp_dir().join(format!("tos-single-report-{}", ulid::Ulid::new()));
    let path = directory.join("report.csv");
    let trace = tos_core::agent::request_id::ServiceRequestTrace::default();
    capture_service_request_id(async {
        trace.record_response(Some("id,\"quoted\""), true);
        write_single_report(path.to_str(), "restore", "source", None, "succeeded").unwrap();
        write_error_report(
            path.to_str(),
            "restore",
            "source",
            None,
            &CliError::ValidationError("decode failed".into()),
        )
        .unwrap();
    })
    .await;
    let report = fs::read_to_string(&path).expect("read single report");
    assert!(report.lines().next().unwrap().ends_with(",request_id"));
    assert!(report
        .lines()
        .skip(1)
        .all(|row| row.ends_with(",\"id,\"\"quoted\"\"\"")));
    fs::remove_dir_all(directory).expect("remove report directory");
}

#[test]
fn manifested_csv_matches_each_action_to_its_service_id() {
    let directory = std::env::temp_dir().join(format!("tos-action-report-{}", ulid::Ulid::new()));
    let path = directory.join("report.csv");
    let mappings = ["a", "b"].map(|name| TransferPlanItem {
        relative_key: name.into(),
        source: format!("tos://source/{name}"),
        destination: format!("tos://destination/{name}"),
        size: 1,
        etag: None,
        crc64: None,
        last_modified: None,
    });
    let manifest = build_transfer_manifest(&mappings);
    let mut report =
        BatchReport::new_streaming(2, path.to_str(), "tos cp", false, Some(&manifest)).unwrap();
    for (index, item) in mappings.iter().enumerate().rev() {
        report.record_copy_result(
            "copy",
            &item.source,
            &item.destination,
            &transferred(Some(&format!("service-{index}"))),
        );
    }
    write_tos_batch_report(path.to_str(), "tos cp", &report, false).unwrap();
    let body = fs::read_to_string(&path).expect("read manifested report");
    let fields = body
        .lines()
        .skip(1)
        .map(|row| row.split(',').collect::<Vec<_>>())
        .collect::<Vec<_>>();
    assert_eq!(
        (&fields[0][9], &fields[0][10], &fields[0][11]),
        (&"3", &"2", &"service-1")
    );
    assert_eq!(
        (&fields[1][9], &fields[1][10], &fields[1][11]),
        (&"3", &"1", &"service-0")
    );
    fs::remove_dir_all(directory).expect("remove report directory");
}

fn transferred(request_id: Option<&str>) -> CopyTransferResult {
    CopyTransferResult {
        outcome: CopyOutcome::Transferred,
        response_data: None,
        request_id: request_id.map(str::to_string),
        status_code: Some(200),
        ec: None,
    }
}

#[test]
fn report_appends_request_id_after_existing_columns() {
    assert_eq!(TOS_REPORT_COLUMNS.last(), Some(&"request_id"));
    assert_eq!(
        &TOS_REPORT_COLUMNS[..11],
        &[
            "command",
            "operation",
            "source",
            "destination",
            "status",
            "error_kind",
            "error_code",
            "message",
            "retryable",
            "schema_version",
            "action_id",
        ]
    );
}

#[test]
fn unmanifested_copy_report_keeps_its_response_id() {
    let mut report = BatchReport::new(1);
    record_tos_stream_copy_result(
        &None,
        &mut report,
        "source".into(),
        "tos://bucket/destination".into(),
        1,
        Ok(transferred(Some("copy-response"))),
    );
    let row = tos_report_record("tos cp", &report.succeeded[0], None);
    assert_eq!(row.last().map(String::as_str), Some("copy-response"));
}

#[test]
fn failed_report_reads_only_the_error_own_service_id() {
    let error = CliError::TransferFailed("HTTP 403 Forbidden (RequestId: denied-response)".into());
    let item = BatchItemResult::failure("copy", "source", None, &error);
    let row = tos_report_record("tos cp", &item, None);
    assert_eq!(row.last().map(String::as_str), Some("denied-response"));
}

#[test]
fn local_failure_report_keeps_request_id_empty() {
    let error = CliError::ValidationError("invalid local input".into());
    let item = BatchItemResult::failure("copy", "source", None, &error);
    let row = tos_report_record("tos cp", &item, None);
    assert_eq!(row.last().map(String::as_str), Some(""));
}

#[test]
fn unsafe_error_service_id_is_omitted() {
    let error = CliError::TransferFailed("HTTP 500 (RequestId: unsafe\nvalue)".into());
    let item = BatchItemResult::failure("copy", "source", None, &error);
    let row = tos_report_record("tos cp", &item, None);
    assert_eq!(row.last().map(String::as_str), Some(""));
}

#[test]
fn unsafe_copy_response_id_is_omitted() {
    let mut report = BatchReport::new(1);
    report.record_copy_result(
        "copy",
        "source",
        "destination",
        &transferred(Some("bad\nid")),
    );
    assert_eq!(report.succeeded[0].request_id, None);
}

#[test]
fn du_uses_the_shared_command_diagnostic_limit() {
    assert_eq!(
        DU_REQUEST_ID_LIMIT,
        tos_core::agent::request_id::SERVICE_REQUEST_ID_LIMIT
    );
}

#[test]
fn legacy_report_header_is_not_mixed_with_new_rows() {
    let directory = std::env::temp_dir().join(format!("tos-report-header-{}", ulid::Ulid::new()));
    fs::create_dir_all(&directory).expect("create report directory");
    let path = directory.join("report.csv");
    let legacy = "command,operation,source,destination,status,error_kind,error_code,message,retryable,schema_version,action_id\n";
    fs::write(&path, legacy).expect("write legacy header");
    let result = write_single_report(path.to_str(), "copy", "source", None, "succeeded");
    assert!(result.is_err());
    assert_eq!(
        fs::read_to_string(&path).expect("read unchanged report"),
        legacy
    );
    fs::remove_dir_all(directory).expect("remove report directory");
}

#[tokio::test]
async fn response_caused_local_failure_keeps_the_file_service_id() {
    let trace = tos_core::agent::request_id::ServiceRequestTrace::default();
    let captured = capture_report_result(async {
        trace.record_response(Some("decode-response"), true);
        Err::<(), _>(CliError::ValidationError("invalid response payload".into()))
    })
    .await;
    let mut report = BatchReport::new(1);
    captured.record(|result| report.record_failure("copy", "source", None, &result.unwrap_err()));
    assert_eq!(
        report.failed[0].request_id.as_deref(),
        Some("decode-response")
    );
}

#[tokio::test]
async fn terminal_transport_failure_leaves_the_file_id_empty() {
    let trace = tos_core::agent::request_id::ServiceRequestTrace::default();
    let captured = capture_report_result(async {
        trace.record_response(Some("retry-response"), false);
        trace.record_no_response();
        Err::<(), _>(CliError::TransferFailed("transport failed".into()))
    })
    .await;
    let mut report = BatchReport::new(1);
    captured.record(|result| report.record_failure("copy", "source", None, &result.unwrap_err()));
    assert_eq!(report.failed[0].request_id, None);
}

#[tokio::test]
async fn report_rows_keep_ids_after_the_command_diagnostic_limit() {
    let trace = tos_core::agent::request_id::ServiceRequestTrace::with_limit(100);
    let mut report = BatchReport::new(105);
    for index in 0..105 {
        let expected_id = format!("file-{index}");
        let captured = capture_report_result(async {
            trace.record_response(Some(&expected_id), true);
            Ok(())
        })
        .await;
        captured.record(|_| report.record_success("copy", &expected_id, None));
    }
    assert_eq!(trace.snapshot().request_ids_omitted, 5);
    assert!(report
        .succeeded
        .iter()
        .enumerate()
        .all(|(index, item)| item.request_id.as_deref() == Some(format!("file-{index}").as_str())));
}

#[tokio::test]
async fn concurrent_file_captures_do_not_exchange_report_ids() {
    let trace = tos_core::agent::request_id::ServiceRequestTrace::default();
    let (first, second) = tokio::join!(
        capture_report_result(async {
            trace.record_response(Some("first-retry"), false);
            tokio::task::yield_now().await;
            trace.record_response(Some("first-final"), true);
            Ok(())
        }),
        capture_report_result(async {
            trace.record_response(Some("second-final"), true);
            Ok(())
        }),
    );
    let mut report = BatchReport::new(2);
    second.record(|_| report.record_success("delete", "second", None));
    first.record(|_| report.record_success("delete", "first", None));
    assert_eq!(
        report.succeeded[0].request_id.as_deref(),
        Some("second-final")
    );
    assert_eq!(
        report.succeeded[1].request_id.as_deref(),
        Some("first-final")
    );
}
