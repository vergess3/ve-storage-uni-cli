//! Bounded remote snapshot and portable, unambiguous local path mapping.
use tos_core::agent::error::CliError;

/// Validate a raw relative API/local path; return an error for unsafe or unbounded components.

pub(crate) fn validate_relative(path: &str) -> Result<(), CliError> {
    if path.is_empty() || path.len() > 4096 || path.split('/').count() > 64 {
        return Err(invalid(
            "download path is empty or exceeds path/depth limit",
        ));
    }
    for component in path.split('/') {
        let base = component.split('.').next().unwrap_or("").to_uppercase();
        let is_device = matches!(base.as_str(), "CON" | "PRN" | "AUX" | "NUL")
            || (base.len() == 4
                && (base.starts_with("COM") || base.starts_with("LPT"))
                && base.as_bytes()[3].is_ascii_digit());
        if component.is_empty()
            || component == "."
            || component == ".."
            || component.ends_with(['.', ' '])
            || is_device
            || component
                .chars()
                .any(|letter| letter.is_control() || "\\:<>\"|?*".contains(letter))
        {
            return Err(invalid("unsafe or non-portable download path"));
        }
    }
    Ok(())
}

fn invalid(message: &str) -> CliError {
    CliError::ValidationError(message.into())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn rejects_paths_that_can_escape_or_alias_on_supported_filesystems() {
        for path in [
            "/a", "../a", "a/../b", "a//b", "a\\b", "a\0b", "C:a", "a/.", "a.", "CON",
        ] {
            assert!(validate_relative(path).is_err(), "accepted {path:?}");
        }
        assert!(validate_relative("中文/100%原样.txt").is_ok());
    }
}

use super::{common::map_ids_error, share_receive::ShareReceiver};
use crate::domain::{share_types::ShareListFilesInput, types::FileInfo};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet, VecDeque};
use unicode_normalization::UnicodeNormalization;

pub(crate) const MAX_MANIFEST_BYTES: usize = 8 * 1024 * 1024;
const MAX_ENTRIES: usize = 20_000;
const MAX_PAGES: usize = 10_000;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct RemoteFile {
    pub(crate) path: String,
    pub(crate) size: u64,
    pub(crate) etag: String,
    pub(crate) crc64: u64,
    pub(crate) updated_at: i64,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Entry {
    pub(crate) local: String,
    pub(crate) remote: RemoteFile,
}
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Manifest {
    pub(crate) roots: Vec<String>,
    pub(crate) files: Vec<Entry>,
    pub(crate) directories: Vec<String>,
    pub(crate) remote_directories: Vec<String>,
}
impl Manifest {
    /// Validate snapshot limits and all remote/local mappings; reject collisions and malformed entries.
    pub(crate) fn validate(&self) -> Result<(), CliError> {
        if self.files.len() + self.directories.len() > MAX_ENTRIES
            || serde_json::to_vec(self)?.len() > MAX_MANIFEST_BYTES
        {
            return Err(invalid("manifest exceeds 20,000 entries or 8 MiB"));
        }
        self.validate_roots_and_directories()?;
        let mut remote_paths = BTreeSet::new();
        let mut paths = BTreeMap::new();
        for directory in &self.directories {
            insert_path(&mut paths, directory, true)?;
        }
        for entry in &self.files {
            validate_relative(&entry.remote.path)?;
            if !remote_paths.insert(&entry.remote.path)
                || map_local(&entry.remote.path, &self.roots)? != entry.local
            {
                return Err(invalid("invalid or duplicate manifest remote mapping"));
            }
            insert_path(&mut paths, &entry.local, false)?;
        }
        Ok(())
    }
    fn validate_roots_and_directories(&self) -> Result<(), CliError> {
        // [Review Fix #Download6] Local directories need the same remote-to-local proof as files.
        if self.remote_directories.len() != self.directories.len() {
            return Err(invalid("directory mapping count differs"));
        }
        let mut seen_roots = BTreeSet::new();
        for root in &self.roots {
            validate_relative(root.strip_suffix('/').unwrap_or(root))?;
            let has_target = if root.ends_with('/') {
                self.remote_directories.contains(root)
            } else {
                self.files.iter().any(|entry| &entry.remote.path == root)
            };
            if !seen_roots.insert(root) || !has_target {
                return Err(invalid("duplicate or unrepresented manifest root"));
            }
        }
        let mut seen_directories = BTreeSet::new();
        let mut seen_local = BTreeSet::new();
        for (remote, local) in self.remote_directories.iter().zip(&self.directories) {
            if !remote.ends_with('/')
                || !seen_directories.insert(remote)
                || !seen_local.insert(local)
            {
                return Err(invalid("invalid or duplicate directory mapping"));
            }
            validate_relative(remote.strip_suffix('/').unwrap_or(remote))?;
            if map_local(remote, &self.roots)? != *local {
                return Err(invalid("directory mapping differs from remote roots"));
            }
        }
        Ok(())
    }
    /// Bind a validated snapshot to the immutable selection and recursion policy; reject impossible task shapes.
    pub(crate) fn validate_selection(
        &self,
        selection: Option<&str>,
        is_recursive: bool,
    ) -> Result<(), CliError> {
        if !is_recursive
            && (!self.directories.is_empty() || self.roots.iter().any(|root| root.ends_with('/')))
        {
            return Err(invalid("directory manifest requires recursive policy"));
        }
        let Some(selection) = selection else {
            return Ok(());
        };
        if self.roots.as_slice() != [selection] {
            return Err(invalid("manifest roots differ from selected target"));
        }
        if selection.ends_with('/') {
            if !is_recursive
                || !self
                    .remote_directories
                    .iter()
                    .any(|directory| directory == selection)
            {
                return Err(invalid("selected directory is absent or nonrecursive"));
            }
        } else if self.files.len() != 1
            || self.files[0].remote.path != selection
            || !self.directories.is_empty()
        {
            return Err(invalid(
                "exact file selection must contain exactly that file and no directories",
            ));
        }
        Ok(())
    }
}
fn insert_path(
    paths: &mut BTreeMap<String, (String, bool)>,
    path: &str,
    is_directory: bool,
) -> Result<(), CliError> {
    validate_relative(path)?;
    let components: Vec<_> = path.split('/').collect();
    for count in 1..=components.len() {
        let original = components[..count].join("/");
        let folded: String = original.nfd().flat_map(char::to_uppercase).collect();
        let is_parent = count < components.len() || is_directory;
        if let Some((previous, was_directory)) = paths.get(&folded) {
            if previous != &original || !was_directory || !is_parent {
                return Err(invalid(
                    "download paths collide (case, Unicode, or file/directory)",
                ));
            }
        } else {
            paths.insert(folded, (original, is_parent));
        }
    }
    Ok(())
}
impl RemoteFile {
    /// Extract the minimal remote fingerprint from metadata, rejecting unsafe paths or invalid sizes.
    pub(crate) fn from_info(info: FileInfo) -> Result<Self, CliError> {
        validate_relative(&info.file_path)?;
        let size = u64::try_from(info.size).map_err(|_| {
            invalid("remote file size is missing or negative; cannot build an exact download plan")
        })?;
        if info.etag.len() > 1024 || info.etag.chars().any(char::is_control) {
            return Err(invalid("invalid remote ETag"));
        }
        Ok(Self {
            path: info.file_path,
            size,
            etag: info.etag,
            crc64: info.hash_crc64_ecma,
            updated_at: info.updated_at,
        })
    }
    /// Return whether the fingerprint has a strong ETag or CRC64 suitable for recovery.
    pub(crate) fn has_identity(&self) -> bool {
        (!self.etag.is_empty() && !self.etag.starts_with("W/")) || self.crc64 != 0
    }
}
fn is_folder(info: &FileInfo) -> bool {
    info.file_type.eq_ignore_ascii_case("folder") || info.file_path.ends_with('/')
}
fn directory_path(path: &str) -> Result<String, CliError> {
    validate_relative(path.strip_suffix('/').unwrap_or(path))?;
    Ok(format!("{}/", path.trim_end_matches('/')))
}

/// Read the selected file/tree through the authorized receiver into a bounded snapshot; fail on inconsistent scope or pagination.

pub(crate) async fn enumerate(
    receiver: &mut ShareReceiver,
    selection: Option<&str>,
    is_recursive: bool,
    declared_roots: Option<&[String]>,
) -> Result<Manifest, CliError> {
    let mut snapshot = Snapshot::default();
    if let Some(path) = selection {
        let info = receiver
            .ensure_session()
            .await?
            .get_file(path)
            .await
            .map_err(map_ids_error)?;
        if !is_folder(&info) {
            snapshot.add_file(info)?;
            snapshot.roots.push(path.into());
            return snapshot.finish();
        }
        if !path.ends_with('/') {
            return Err(invalid("directory selection must end with a slash"));
        }
        snapshot.add_directory(path)?;
        snapshot.roots.push(directory_path(path)?);
    } else {
        if let Some(roots) = declared_roots.filter(|roots| !roots.is_empty()) {
            snapshot.roots = roots.to_vec();
            for root in roots.iter().filter(|root| root.ends_with('/')) {
                snapshot.add_directory(root)?;
            }
        }
        snapshot.read_directory(receiver, None).await?;
        if snapshot.roots.is_empty() {
            snapshot.roots.extend(snapshot.directories.iter().cloned());
            snapshot.roots.extend(
                snapshot
                    .files
                    .keys()
                    .filter(|path| !snapshot.directories.iter().any(|dir| path.starts_with(dir)))
                    .cloned(),
            );
        }
    }
    if !snapshot.directories.is_empty() && !is_recursive {
        return Err(invalid("directory downloads require --recursive"));
    }
    while let Some(directory) = snapshot.queue.pop_front() {
        snapshot.read_directory(receiver, Some(&directory)).await?;
    }
    snapshot.finish()
}
#[derive(Default)]
struct Snapshot {
    roots: Vec<String>,
    files: BTreeMap<String, RemoteFile>,
    directories: BTreeSet<String>,
    queue: VecDeque<String>,
    pages: usize,
    encoded_bytes: usize,
    directory_versions: BTreeMap<String, i64>,
}
impl Snapshot {
    fn check_limit(&self) -> Result<(), CliError> {
        if self.files.len() + self.directories.len() > MAX_ENTRIES
            || self.pages > MAX_PAGES
            || self.encoded_bytes > MAX_MANIFEST_BYTES
        {
            return Err(invalid(
                "enumeration exceeds 20,000 entries, 10,000 pages or 8 MiB",
            ));
        }
        Ok(())
    }
    fn add_directory(&mut self, path: &str) -> Result<(), CliError> {
        let directory = directory_path(path)?;
        if self.directories.insert(directory.clone()) {
            self.encoded_bytes += directory.len() + 64;
            self.queue.push_back(directory);
        }
        self.check_limit()
    }
    fn add_file(&mut self, info: FileInfo) -> Result<(), CliError> {
        if is_folder(&info) {
            self.directory_version(&info.file_path, info.updated_at)?;
            return self.add_directory(&info.file_path);
        }
        let file = RemoteFile::from_info(info)?;
        if self
            .files
            .get(&file.path)
            .is_some_and(|previous| previous != &file)
        {
            return Err(invalid(
                "remote file changed or conflicting duplicate during enumeration",
            ));
        }
        if !self.files.contains_key(&file.path) {
            self.encoded_bytes += file.path.len() * 2 + file.etag.len() * 2 + 256;
        }
        self.files.insert(file.path.clone(), file);
        self.check_limit()
    }
    fn directory_version(&mut self, path: &str, version: i64) -> Result<(), CliError> {
        let path = directory_path(path)?;
        if version == 0 {
            return Ok(());
        }
        if self
            .directory_versions
            .get(&path)
            .is_some_and(|previous| *previous != version)
        {
            return Err(invalid("conflicting duplicate directory metadata"));
        }
        self.directory_versions.insert(path, version);
        Ok(())
    }
    async fn read_directory(
        &mut self,
        receiver: &mut ShareReceiver,
        prefix: Option<&str>,
    ) -> Result<(), CliError> {
        let mut marker = None;
        let mut markers = BTreeSet::new();
        loop {
            self.pages += 1;
            self.check_limit()?;
            let page = receiver
                .ensure_session()
                .await?
                .list_files(&ShareListFilesInput {
                    prefix: prefix.map(str::to_owned),
                    delimiter: Some("/".into()),
                    marker: marker.clone(),
                    limit: Some(1000),
                    list_root: Some(prefix.is_none()),
                })
                .await
                .map_err(map_ids_error)?;
            for folder in page.folders {
                check_scope(&folder.folder, prefix)?;
                self.directory_version(&folder.folder, folder.updated_at)?;
                self.add_directory(&folder.folder)?;
            }
            for file in page.files {
                check_scope(&file.file_path, prefix)?;
                self.add_file(file)?;
            }
            if page.next_marker.is_empty() {
                if page.is_truncated {
                    return Err(invalid("truncated page is missing NextMarker"));
                }
                break;
            }
            if page.next_marker.len() > 4096 {
                return Err(invalid("pagination marker exceeds 4096 bytes"));
            }
            if !markers.insert(page.next_marker.clone()) {
                return Err(invalid("cyclic pagination marker"));
            }
            marker = Some(page.next_marker);
        }
        Ok(())
    }
    fn finish(self) -> Result<Manifest, CliError> {
        let manifest = Manifest {
            roots: self.roots.clone(),
            remote_directories: self.directories.iter().cloned().collect(),
            files: self
                .files
                .into_values()
                .map(|remote| {
                    Ok(Entry {
                        local: map_local(&remote.path, &self.roots)?,
                        remote,
                    })
                })
                .collect::<Result<_, CliError>>()?,
            directories: self
                .directories
                .iter()
                .map(|path| map_local(path, &self.roots))
                .collect::<Result<_, _>>()?,
        };
        manifest.validate()?;
        Ok(manifest)
    }
}
fn check_scope(path: &str, prefix: Option<&str>) -> Result<(), CliError> {
    if prefix.is_some_and(|scope| !path.starts_with(scope)) {
        return Err(invalid(
            "service returned a path outside the selected directory",
        ));
    }
    Ok(())
}
fn map_local(path: &str, roots: &[String]) -> Result<String, CliError> {
    let root = roots
        .iter()
        .filter(|root| {
            path == root.as_str() || (root.ends_with('/') && path.starts_with(root.as_str()))
        })
        .min_by_key(|root| root.len())
        .ok_or_else(|| invalid("path is outside manifest roots"))?;
    let root_name = root
        .trim_end_matches('/')
        .rsplit('/')
        .next()
        .ok_or_else(|| invalid("empty root"))?;
    let suffix = path.strip_prefix(root).unwrap_or("").trim_end_matches('/');
    let local = if suffix.is_empty() {
        root_name.into()
    } else {
        format!("{root_name}/{suffix}")
    };
    validate_relative(&local)?;
    Ok(local)
}
#[cfg(test)]
mod snapshot_tests {
    use super::*;
    use crate::domain::share_access::fixture::{reply, Fixture};
    use tos_core::agent::global_args::GlobalArgs;
    #[test]
    fn rejects_case_normalization_and_ancestor_collisions() {
        for paths in [["A", "a"], ["é", "e\u{301}"], ["a", "a/b"]] {
            let manifest = Manifest {
                roots: paths.map(str::to_owned).into(),
                files: paths
                    .map(|path| Entry {
                        local: path.into(),
                        remote: RemoteFile {
                            path: path.into(),
                            size: 0,
                            etag: "tag".into(),
                            crc64: 0,
                            updated_at: 0,
                        },
                    })
                    .into(),
                directories: vec![],
                remote_directories: vec![],
            };
            assert!(manifest.validate().is_err(), "accepted {paths:?}");
        }
    }
    #[tokio::test]
    async fn exact_selection_maps_basename_once() {
        let fixture = Fixture::new(vec![
            reply(200, r#"{"ShareID":"s"}"#),
            reply(200, r#"{"AccessToken":"token","ExpiresIn":60}"#),
            reply(
                200,
                r#"{"FilePath":"parent/中文%.txt","Size":3,"ETag":"tag"}"#,
            ),
        ]);
        let global = GlobalArgs::default();
        let mut receiver =
            ShareReceiver::new(&global, &format!("{}/share/s", fixture.endpoint)).unwrap();
        let info = receiver.info().await.unwrap();
        receiver.authorize(&global, &info, false).await.unwrap();
        let manifest = enumerate(&mut receiver, Some("parent/中文%.txt"), false, None)
            .await
            .unwrap();
        assert_eq!(manifest.files.len(), 1);
        assert_eq!(manifest.files[0].local, "中文%.txt");
        fixture.finish();
    }
}

#[cfg(test)]
mod tree_tests {
    use super::*;
    use crate::domain::share_access::fixture::{reply, Fixture};
    use tos_core::agent::global_args::GlobalArgs;
    async fn receiver(fixture: &Fixture) -> ShareReceiver {
        let global = GlobalArgs::default();
        let mut receiver =
            ShareReceiver::new(&global, &format!("{}/share/s", fixture.endpoint)).unwrap();
        let info = receiver.info().await.unwrap();
        receiver.authorize(&global, &info, false).await.unwrap();
        receiver
    }
    fn responses(pages: &[&str]) -> Vec<String> {
        let mut responses = vec![
            reply(200, r#"{"ShareID":"s"}"#),
            reply(200, r#"{"AccessToken":"t","ExpiresIn":60}"#),
        ];
        responses.extend(pages.iter().map(|page| reply(200, page)));
        responses
    }
    #[tokio::test]
    async fn root_tree_retains_empty_folders_pages_and_subtree_duplicates() {
        let fixture = Fixture::new(responses(&[
            r#"{"Folders":[{"Folder":"parent/root/"}],"NextMarker":"p2"}"#,
            r#"{"Files":[{"FilePath":"parent/root/a","Size":1,"ETag":"a"}]}"#,
            r#"{"Files":[{"FilePath":"parent/root/a","Size":1,"ETag":"a"},{"FilePath":"parent/root/empty/","FileType":"folder"}]}"#,
            r#"{"Files":[],"Folders":[]}"#,
        ]));
        let manifest = enumerate(&mut receiver(&fixture).await, None, true, None)
            .await
            .unwrap();
        assert_eq!(manifest.files.len(), 1);
        assert_eq!(manifest.files[0].local, "root/a");
        assert_eq!(manifest.directories, vec!["root", "root/empty"]);
        let requests = fixture.finish();
        assert!(requests[3].contains("marker=p2"));
        assert_eq!(requests.len(), 6);
    }
    #[tokio::test]
    async fn explicit_directory_preserves_name_once_and_requires_existence() {
        let fixture = Fixture::new(responses(&[
            r#"{"FilePath":"parent/child/","FileType":"folder"}"#,
            r#"{"Files":[]}"#,
        ]));
        let manifest = enumerate(
            &mut receiver(&fixture).await,
            Some("parent/child/"),
            true,
            None,
        )
        .await
        .unwrap();
        assert_eq!(manifest.directories, vec!["child"]);
        assert_eq!(fixture.finish().len(), 4);
        let fixture = Fixture::new(responses(&["{}"]));
        assert!(
            enumerate(&mut receiver(&fixture).await, Some("missing/"), true, None)
                .await
                .is_err()
        );
        fixture.finish();
    }
    #[tokio::test]
    async fn malformed_pagination_never_silently_truncates() {
        for pages in [
            vec![r#"{"IsTruncated":true}"#],
            vec![r#"{"NextMarker":"x"}"#, r#"{"NextMarker":"x"}"#],
        ] {
            let fixture = Fixture::new(responses(&pages));
            assert!(enumerate(&mut receiver(&fixture).await, None, true, None)
                .await
                .is_err());
            fixture.finish();
        }
    }
    #[tokio::test]
    async fn selected_subtree_rejects_out_of_scope_response() {
        let fixture = Fixture::new(responses(&[
            r#"{"FilePath":"selected/","FileType":"folder"}"#,
            r#"{"Files":[{"FilePath":"other/file","Size":1}]}"#,
        ]));
        assert!(
            enumerate(&mut receiver(&fixture).await, Some("selected/"), true, None)
                .await
                .is_err()
        );
        fixture.finish();
    }
}
