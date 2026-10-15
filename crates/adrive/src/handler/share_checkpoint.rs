//! Versioned, bounded, secret-free file-level recovery journal.
use super::{
    share_local::{resolve_directory, SafeDir, StagedFile},
    share_manifest::{Manifest, MAX_MANIFEST_BYTES},
};
use fs2::FileExt;
use serde::{Deserialize, Serialize};
use std::{
    fs::File,
    io::{Read, Write},
    path::{Path, PathBuf},
};
use tos_core::agent::error::CliError;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Identity {
    pub(crate) origin: String,
    pub(crate) share_id: String,
    pub(crate) selection: Option<String>,
    pub(crate) destination: String,
    pub(crate) is_recursive: bool,
    pub(crate) is_overwrite: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) api_origin: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) recipient: Option<super::share_receiver_auth::ReceiverContext>,
}
impl Identity {
    fn matches_expected(&self, expected: &Self) -> Result<bool, CliError> {
        let mut recorded = self.clone();
        // [Review Fix #ShareReceiveResume1] Legacy anonymous journals may migrate only to the URL-derived API; a new/overridden API must be recorded explicitly.
        if recorded.api_origin.is_none()
            && recorded.recipient.is_none()
            && expected.api_origin.is_some()
        {
            recorded.api_origin = Some(
                crate::domain::share_url::derive_share_api_origin(&recorded.origin)
                    .map_err(super::common::map_ids_error)?,
            );
        }
        Ok(&recorded == expected)
    }
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Journal {
    pub(crate) version: u32,
    pub(crate) identity: Identity,
    pub(crate) manifest: Manifest,
    pub(crate) completed: Vec<Option<String>>,
}
impl Journal {
    /// Check this untrusted journal against the expected identity and strict manifest/completion invariants; return validation errors.
    pub(crate) fn validate(&self, identity: &Identity) -> Result<(), CliError> {
        if self.version != 1 || !self.identity.matches_expected(identity)? {
            return Err(invalid(
                "checkpoint version or task identity/policy differs",
            ));
        }
        self.manifest.validate()?;
        if self.completed.len() != self.manifest.files.len()
            || self.completed.iter().flatten().any(|digest| {
                digest.len() != 64 || !digest.bytes().all(|byte| byte.is_ascii_hexdigit())
            })
        {
            return Err(invalid("invalid checkpoint completion records"));
        }
        // [Review Fix #Download6] The checkpoint may not redefine roots or directory shape around an immutable selector.
        self.manifest
            .validate_selection(identity.selection.as_deref(), identity.is_recursive)?;
        Ok(())
    }
}
fn invalid(message: &str) -> CliError {
    CliError::ValidationError(message.into())
}

pub(crate) struct Checkpoint {
    directory: SafeDir,
    name: String,
    resolved_path: PathBuf,
    _lock: File,
    has_saved: std::cell::Cell<bool>,
}
impl Checkpoint {
    /// Acquire a no-follow checkpoint sidecar lock and enforce fresh/resume existence policy; propagate conflicts and I/O errors.
    pub(crate) fn open(path: &Path, is_resume: bool) -> Result<Self, CliError> {
        let name = path
            .file_name()
            .and_then(|name| name.to_str())
            .ok_or_else(|| invalid("invalid checkpoint filename"))?
            .to_owned();
        super::share_manifest::validate_relative(&name)?;
        let parent = path
            .parent()
            .filter(|path| !path.as_os_str().is_empty())
            .unwrap_or(Path::new("."));
        // [Review Fix #LiveRoot2] Canonical parent identity prevents aliases from bypassing checkpoint/lock collision checks.
        let parent = resolve_directory(parent, true)?;
        let directory = SafeDir::open_resolved(&parent)?;
        let lock_name = format!("{name}.lock");
        let lock = match directory.exclusive(&lock_name) {
            Ok(lock) => lock,
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {
                directory.read(&lock_name)?
            }
            Err(error) => return Err(error.into()),
        };
        lock.try_lock_exclusive()
            .map_err(|_| CliError::Conflict("checkpoint is locked by another download".into()))?;
        let has_checkpoint = directory.ordinary(&name)?;
        if has_checkpoint != is_resume {
            return Err(invalid(if has_checkpoint {
                "checkpoint exists; use --resume"
            } else {
                "resume checkpoint does not exist"
            }));
        }
        Ok(Self {
            directory,
            resolved_path: parent.join(&name),
            name,
            _lock: lock,
            has_saved: std::cell::Cell::new(is_resume),
        })
    }
    /// Return the resolved local checkpoint identity without following its protected leaf.
    pub(crate) fn path(&self) -> &Path {
        &self.resolved_path
    }
    /// Read bounded strict JSON and validate it against the requested task identity; return the original journal or an error.
    pub(crate) fn load(&self, identity: &Identity) -> Result<Journal, CliError> {
        let mut bytes = Vec::new();
        self.directory
            .read(&self.name)?
            .take((MAX_MANIFEST_BYTES + 1) as u64)
            .read_to_end(&mut bytes)?;
        if bytes.len() > MAX_MANIFEST_BYTES {
            return Err(invalid("checkpoint exceeds 8 MiB"));
        }
        let mut journal: Journal = serde_json::from_slice(&bytes)
            .map_err(|_| invalid("invalid checkpoint JSON/schema"))?;
        journal.validate(identity)?;
        // [Review Fix #ShareReceiveResume1] Persist the explicit API identity after successful legacy validation.
        journal.identity = identity.clone();
        Ok(journal)
    }
    /// Atomically persist the validated secret-free journal with file synchronization; reject initial replacement and propagate failures.
    pub(crate) fn save(&self, journal: &Journal) -> Result<(), CliError> {
        journal.validate(&journal.identity)?;
        let bytes = serde_json::to_vec(journal)?;
        if bytes.len() > MAX_MANIFEST_BYTES {
            return Err(invalid("checkpoint exceeds 8 MiB"));
        }
        let mut staged = StagedFile::new(&self.directory, &self.name)?;
        staged.file.write_all(&bytes)?;
        // [Review Fix #Download4] Initial persistence must not overwrite an unrelated concurrently created checkpoint.
        staged.publish(self.has_saved.get())?;
        self.has_saved.set(true);
        Ok(())
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    fn identity() -> Identity {
        Identity {
            origin: "https://example.test".into(),
            share_id: "s".into(),
            selection: None,
            destination: "/dest".into(),
            is_recursive: true,
            is_overwrite: false,
            recipient: None,
            api_origin: None,
        }
    }
    #[test]
    fn receiver_context_is_optional_but_changes_checkpoint_identity() {
        let legacy = identity();
        let mut encoded = serde_json::to_value(&legacy).unwrap();
        assert_eq!(
            serde_json::from_value::<Identity>(encoded.clone()).unwrap(),
            legacy
        );
        encoded["recipient"] =
            serde_json::json!({"api_origin":"https://api.test","profile":"receiver","user_id":"u"});
        let authenticated = serde_json::from_value::<Identity>(encoded).unwrap();
        assert_ne!(authenticated, legacy);
    }

    #[test]
    fn legacy_anonymous_checkpoint_migrates_only_to_derived_api() {
        let mut legacy = identity();
        legacy.origin = "http://inst.idsapp-boe.volces.com".into();
        let mut expected = legacy.clone();
        expected.api_origin = Some("http://inst.idsapi-boe.volces.com".into());
        assert!(legacy.matches_expected(&expected).unwrap());
        expected.api_origin = Some("http://different-api.test".into());
        assert!(!legacy.matches_expected(&expected).unwrap());
    }

    #[test]
    fn actual_api_origin_participates_in_anonymous_recovery_identity() {
        let mut encoded = serde_json::to_value(identity()).unwrap();
        encoded["api_origin"] = serde_json::json!("https://api-a.test");
        let first = serde_json::from_value::<Identity>(encoded.clone()).unwrap();
        encoded["api_origin"] = serde_json::json!("https://api-b.test");
        let second = serde_json::from_value::<Identity>(encoded).unwrap();
        assert_ne!(first, second);
    }
    #[test]
    fn rejects_unknown_version_policy_and_completion_length() {
        let mut journal = Journal {
            version: 2,
            identity: identity(),
            manifest: Manifest::default(),
            completed: vec![],
        };
        assert!(journal.validate(&identity()).is_err());
        journal.version = 1;
        journal.identity.is_overwrite = true;
        assert!(journal.validate(&identity()).is_err());
        journal.identity = identity();
        journal.completed.push(Some("bad".into()));
        assert!(journal.validate(&identity()).is_err());
    }
}

#[cfg(test)]
mod persistence_tests {
    use super::*;
    fn identity(root: &Path) -> Identity {
        Identity {
            origin: "https://example.test".into(),
            share_id: "s".into(),
            selection: None,
            destination: root.to_string_lossy().into(),
            is_recursive: true,
            is_overwrite: false,
            recipient: None,
            api_origin: None,
        }
    }
    #[test]
    fn locking_strict_json_and_unknown_creation_do_not_clobber_another_task() {
        let root = std::env::temp_dir()
            .canonicalize()
            .unwrap()
            .join(format!("share-checkpoint-{}", ulid::Ulid::new()));
        std::fs::create_dir(&root).unwrap();
        let path = root.join("checkpoint.json");
        let checkpoint = Checkpoint::open(&path, false).unwrap();
        assert!(Checkpoint::open(&path, false).is_err());
        std::fs::write(&path, "another task").unwrap();
        let journal = Journal {
            version: 1,
            identity: identity(&root),
            manifest: Manifest::default(),
            completed: vec![],
        };
        assert!(
            checkpoint.save(&journal).is_err(),
            "first journal save must not overwrite a concurrently created task"
        );
        assert_eq!(std::fs::read_to_string(&path).unwrap(), "another task");
        drop(checkpoint);
        std::fs::write(&path, r#"{"version":999,"unexpected":"field"}"#).unwrap();
        let checkpoint = Checkpoint::open(&path, true).unwrap();
        assert!(checkpoint.load(&identity(&root)).is_err());
        drop(checkpoint);
        std::fs::remove_dir_all(root).unwrap();
    }
}

#[cfg(test)]
mod selection_tests {
    use super::*;
    use crate::handler::share_manifest::{Entry, RemoteFile};
    fn selected(selection: &str, is_recursive: bool) -> Journal {
        Journal {
            version: 1,
            identity: Identity {
                origin: "https://example.test".into(),
                share_id: "s".into(),
                selection: Some(selection.into()),
                destination: "/dest".into(),
                is_recursive,
                is_overwrite: false,
                recipient: None,
                api_origin: None,
            },
            manifest: Manifest::default(),
            completed: vec![],
        }
    }
    #[test]
    fn exact_selection_rejects_rebased_mapping_and_missing_file() {
        let mut journal = selected("d/a", false);
        journal.manifest.roots = vec!["d/".into()];
        journal.manifest.files = vec![Entry {
            local: "d/a".into(),
            remote: RemoteFile {
                path: "d/a".into(),
                size: 0,
                etag: "tag".into(),
                crc64: 0,
                updated_at: 1,
            },
        }];
        journal.completed = vec![None];
        assert!(
            journal.validate(&journal.identity).is_err(),
            "exact file mapping was rebased"
        );
        journal.manifest = Manifest::default();
        journal.completed.clear();
        assert!(
            journal.validate(&journal.identity).is_err(),
            "exact selection cannot become an empty plan"
        );
    }
    #[test]
    fn directory_selection_rejects_arbitrary_directories_and_nonrecursive_shape() {
        let mut journal = selected("d/root/", true);
        journal.manifest.roots = vec!["d/root/".into()];
        journal.manifest.remote_directories = vec!["d/root/".into()];
        journal.manifest.directories = vec!["root".into()];
        assert!(journal.validate(&journal.identity).is_ok());
        journal.manifest.directories = vec!["injected".into()];
        assert!(
            journal.validate(&journal.identity).is_err(),
            "unrelated local directory accepted"
        );
        journal.manifest.directories = vec!["root".into()];
        journal.identity.is_recursive = false;
        assert!(
            journal.validate(&journal.identity).is_err(),
            "directory allowed without recursive"
        );
        journal.identity.is_recursive = true;
        journal.manifest = Manifest::default();
        assert!(
            journal.validate(&journal.identity).is_err(),
            "selected directory disappeared from empty plan"
        );
    }
    #[test]
    fn root_selection_cannot_create_directories_without_matching_remote_roots() {
        let mut journal = selected("unused", true);
        journal.identity.selection = None;
        journal.manifest.directories = vec!["injected".into()];
        assert!(journal.validate(&journal.identity).is_err());
    }
}
