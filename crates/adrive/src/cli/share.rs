//! Share-link management and receiver commands with explicit OAuth opt-in.
use clap::{Args, Subcommand, ValueEnum};

pub(crate) const SHARE_VIEW_HELP: &str = "view previews one exact file; a single shared file is selected when --path is omitted. Folders and multiple roots require --path to a file. Default auto mode returns a short-lived PDF PreviewURL for supported documents, or raw content metadata for other files; it never opens a browser. --raw streams original bytes to a pipe or redirected stdout (for example | less), rejects terminal stdout and conflicts with --output/--query. Preview requests consume PreviewLimit and are never automatically retried. Document URLs are temporary capabilities; keep them private. Unsupported deployments can return 404. Password precedence is stdin > URL pwd > hidden TTY prompt; --password-stdin reads to EOF.";

pub(crate) const DEFAULT_CREATE_SHARE_PERMISSION: &str = "view";
pub(crate) const SHARE_SETTINGS_HELP: &str = "Create defaults to view (preview only); use --permission download to allow downloads. Update preserves omitted settings. SharePwd and RequireLogin are server fields: --password-stdin sets a share password in create/update and supplies it in files/view/download. RequireLogin controls receiver login; management uses the selected account. Receivers default to anonymous; explicit --auth-mode oauth uses the selected recipient profile.";

pub(crate) const SHARE_CONNECTION_HELP: &str = "Standard IDS hosts map between idsapi/idsapp and idsapi-boe/idsapp-boe under volces.com, preserving the Instance label, scheme and port. Share pages, receiver APIs and download addresses support HTTP and HTTPS without protocol upgrades. Override custom page origins with --share-endpoint and receiver API origins with --endpoint. Anonymous receivers derive APIs from page URLs without account config; explicit OAuth uses the selected profile API or --endpoint.";
pub(crate) const SHARE_RECEIVER_HELP: &str = "inspect/files/view/download default to anonymous even if a profile configures OAuth. Explicit --auth-mode oauth reuses --profile via auth login/status and existing token refresh; it never starts login or falls back to another identity. OAuth authenticates share info/token exchange; file APIs use only x-share-token and content downloads carry no account credentials. Login and share permissions are validated by the service; passwords are still required when configured. Public URLs accept /share/<id> or /share/<id>/; one trailing slash is normalized away. Select directory prefixes with --path, not additional URL segments.";
// [Review Fix #ShareFilesCap1] Files expose the same result cap as ls, independent of service page sizes.
pub(crate) const SHARE_FILES_HELP: &str = "files lists the current directory level like ls. --max-keys (default 1000) caps files and folders together; service pages are fetched automatically until the cap or listing end. Continue with --marker using the previous NextMarker; IsTruncated=false means the listing ended. --limit is not supported. Exact file paths select one file; directory paths end in /. Each invocation needs the extraction code when configured: stdin > URL pwd > hidden ordinary-TTY prompt. --password-stdin reads to EOF (use a pipe); passwords are not saved. Default table/CSV uses the same file_path, size, file_type, updated_at and is_folder columns as ls, with folders first. Folder markers, self markers and duplicates are normalized in all output modes; Total and pagination.total_returned count the same logical entries. JSON and explicit --query retain the aggregated normalized Files/Folders metadata.";
// [Review Fix #ShareDownloadHelp1] Preserve shared selection and password guidance without advertising files-only pagination flags.
pub(crate) const SHARE_DOWNLOAD_SELECTION_HELP: &str = "Downloads enumerate all selected service pages automatically. Exact file paths select one file; directory paths end in / and require --recursive. Password precedence is stdin > URL pwd > hidden ordinary-TTY prompt; --password-stdin reads to EOF (use a pipe), and passwords are not saved.";
// [Review Fix #D03D04D06Docs] Local conflicts need local remedies; an ambiguous service denial cannot establish a unique cause.
pub(crate) const SHARE_DOWNLOAD_HELP: &str = "The OS resolves user-selected destination and checkpoint directory aliases before the CLI pins them; downloaded subpaths still reject symlinks/reparse points with Conflict (exit 9) and guidance to choose an ordinary local destination. Resume compares resolved roots; changed completed local file sizes or digests also fail with Conflict (exit 9). Explicit service denials such as 403 retain their error code and set IssuanceOutcomeUnknown=false; ambiguous URL issuance is never automatically replayed. No --path selects all shared roots; a single-file share is selected automatically, while folders require --recursive. Actual downloads create directories, temporary files and, when requested, checkpoint and lock files; preflight can leave empty directories before a later transfer denial. --overwrite replaces existing ordinary files only after verification. Download URL issuance may consume download-count quota even if content transfer fails. The default download table shows a cp-style summary (total/succeeded/failed/skipped/pending), including empty results; JSON and explicit queries retain file details. Summary.Skipped counts validated completed files reused during resume and is included in Summary.Completed. Files at least 20MB with a strong ETag or CRC64 automatically use parallel 16MiB ranges without requiring a checkpoint; files without either identity use one streamed GET to avoid mixing versions; --checkpoint-threshold changes the threshold and --multipart-concurrency changes the default 4 concurrent ranges. Range retries reuse the same issued URL; completed-file resume remains file-level. Scanning and file-count progress use stderr, with per-file range completion shown during large-file transfers; enabled on TTY by default; --progress forces progress, while --no-progress or --quiet disables it. --progress and --no-progress are mutually exclusive. Forbidden.ShareDownloadDisabled may mean the share permission forbids downloads or its download-count quota is exhausted; the service code alone cannot distinguish them. Ask the owner to check permission and quota; preserve the service code, HTTP status and request ID.";
// [Review Fix #ShareReviewDocs] Keep read-only planning and explicit input validation visible in every discovery surface.
pub(crate) const SHARE_DRY_RUN_HELP: &str = "Download --dry-run uses read-only share info/token/list/get metadata requests to enumerate every selected page and recursive folder, or validate the saved resume snapshot and completed local file digests. It returns selection.Manifest, PlanComplete=true, FileCount, DirectoryCount, TotalBytes, SkippedCount and Conflicts. Completed plans with local conflicts exit 9 and retain the plan. DownloadsIssued=0; LocalWrites=false means no local writes: no URL issuance, content fetch, directory creation, temporary file, checkpoint or lock writes. MetadataResponses counts received metadata responses; NetworkRequests=null because wire attempts are not counted. SnapshotGuaranteed=false: remote content and local paths can change after inspection. Missing or negative file sizes fail validation (exit 6); explicit zero is a valid empty file. OAuth dry-run disables automatic refresh to avoid credential and auth-lock writes. If the token is expired, within 60 seconds of expiry, or rejected with 401, first run ve-adrive --profile <profile> share inspect <share-url> --auth-mode oauth as a normal OAuth read to refresh, then retry. Planning never acquires the checkpoint lock; an existing regular sidecar may be active. Execution rechecks state and acquires its lock.";
pub(crate) const SHARE_PASSWORD_HELP: &str = "Explicit --password-stdin must contain a nonempty password after removing one terminal LF or CRLF; EOF-only, LF-only and CRLF-only input fail validation (exit 6). Significant spaces are preserved. Omitting the flag leaves an existing password unchanged on update. create/update --dry-run also reads and validates explicit stdin; --describe and --help never read stdin.";
pub(crate) const SHARE_OFFLINE_HELP: &str = "--describe and --help do not load config, access the network or read stdin. Share --dry-run is offline except download metadata planning. Explicit create/update --password-stdin is read and validated during dry-run.";
pub(crate) const SHARE_SELECTOR_HELP: &str = "get/update/cancel accept adrive://instance/share-id or a bare ShareID with --instance. A repeated --instance must match the URI; --by-name resolves only the instance. Share URIs use one ShareID segment and decode components once; file/space targets are interpreted by their own command context.";
pub(crate) const SHARE_LIST_HELP: &str = "list accepts adrive://instance (with an optional root slash) or --instance instance. A repeated --instance must match the URI; --by-name resolves the instance name. The instance URI has no share/space/file path and decodes once. List returns one page; --creator is optional only when OAuth supplies a bound user_id.";

/// Share-link command group; an omitted action displays group help.
#[derive(Debug, Args)]
pub struct ShareCommand {
    #[command(subcommand)]
    pub action: Option<ShareAction>,
}

/// Supported share operations.
#[derive(Debug, Subcommand)]
pub enum ShareAction {
    /// Create a share link
    #[command(after_help = format!("{SHARE_CONNECTION_HELP}\n\n{SHARE_SETTINGS_HELP}\n\n{SHARE_PASSWORD_HELP}\n\n{SHARE_OFFLINE_HELP}"))]
    Create(CreateShareArgs),
    /// Get a share link
    #[command(after_help = format!("{SHARE_CONNECTION_HELP}\n\n{SHARE_SELECTOR_HELP}\n\n{SHARE_OFFLINE_HELP}"))]
    Get(ShareIdArgs),
    /// List share links
    #[command(after_help = format!("{SHARE_CONNECTION_HELP}\n\n{SHARE_LIST_HELP}\n\n{SHARE_OFFLINE_HELP}"))]
    List(ListShareArgs),
    /// Update share settings
    #[command(after_help = format!("{SHARE_CONNECTION_HELP}\n\n{SHARE_SELECTOR_HELP}\n\n{SHARE_SETTINGS_HELP}\n\n{SHARE_PASSWORD_HELP}\n\n{SHARE_OFFLINE_HELP}"))]
    Update(UpdateShareArgs),
    /// Cancel a share link
    #[command(after_help = format!("{SHARE_SELECTOR_HELP}\n\n{SHARE_OFFLINE_HELP}"))]
    Cancel(CancelShareArgs),
    /// Inspect share info
    #[command(after_help = format!("{SHARE_CONNECTION_HELP}\n\n{SHARE_RECEIVER_HELP}\n\n{SHARE_OFFLINE_HELP}"))]
    Inspect(InspectShareArgs),
    /// List shared files
    #[command(after_help = format!("{SHARE_CONNECTION_HELP}\n\n{SHARE_RECEIVER_HELP}\n\n{SHARE_FILES_HELP}\n\n{SHARE_PASSWORD_HELP}\n\n{SHARE_OFFLINE_HELP}"))]
    Files(ShareFilesArgs),
    /// Preview one shared file
    #[command(after_help = format!("{SHARE_CONNECTION_HELP}\n\n{SHARE_RECEIVER_HELP}\n\n{SHARE_VIEW_HELP}\n\n{SHARE_PASSWORD_HELP}\n\n{SHARE_OFFLINE_HELP}"))]
    View(ShareViewArgs),
    /// Download shared files
    #[command(after_help = format!("{SHARE_CONNECTION_HELP}\n\n{SHARE_RECEIVER_HELP}\n\n{SHARE_DOWNLOAD_SELECTION_HELP}\n\n{SHARE_DOWNLOAD_HELP}\n\n{SHARE_DRY_RUN_HELP}\n\n{SHARE_PASSWORD_HELP}\n\n{SHARE_OFFLINE_HELP}"))]
    Download(ShareDownloadArgs),
}

impl ShareAction {
    /// Return the canonical leaf name without user arguments.
    pub fn name(&self) -> &'static str {
        match self {
            Self::Create(_) => "create",
            Self::Get(_) => "get",
            Self::List(_) => "list",
            Self::Update(_) => "update",
            Self::Cancel(_) => "cancel",
            Self::Inspect(_) => "inspect",
            Self::Files(_) => "files",
            Self::View(_) => "view",
            Self::Download(_) => "download",
        }
    }
}

/// Permission granted by a share link.
#[derive(Debug, Clone, Copy, ValueEnum)]
pub enum SharePermission {
    View,
    Download,
}
impl SharePermission {
    /// Return the V1 wire value.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::View => "view",
            Self::Download => "download",
        }
    }
}

/// Searchable lifecycle status.
#[derive(Debug, Clone, Copy, ValueEnum)]
pub enum ShareStatus {
    Active,
    Expired,
    Revoked,
}
impl ShareStatus {
    /// Return the V1 wire value.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Active => "active",
            Self::Expired => "expired",
            Self::Revoked => "revoked",
        }
    }
}

/// Optional mutable share settings; explicit zero, false and empty strings are retained.
#[derive(Debug, Clone, Default, Args)]
pub struct ShareSettings {
    /// Share name
    #[arg(long)]
    pub name: Option<String>,
    /// Share description
    #[arg(long)]
    pub description: Option<String>,
    /// Share permission: view or download
    #[arg(long, value_enum)]
    pub permission: Option<SharePermission>,
    /// Share lifetime duration; 0 means never expires
    #[arg(long)]
    pub expires_in: Option<String>,
    /// Read one share password from stdin (EOF; use a pipe or Enter then Ctrl-D on macOS/Linux)
    #[arg(long = "password-stdin")]
    pub is_password_stdin: bool,
    /// Require receiver login; accepts explicit true or false
    #[arg(long = "require-login", action = clap::ArgAction::Set)]
    pub is_login_required: Option<bool>,
    /// Maximum preview count; 0 means unlimited
    #[arg(long, value_parser = clap::value_parser!(i64).range(0..))]
    pub preview_limit: Option<i64>,
    /// Maximum download count; 0 means unlimited
    #[arg(long, value_parser = clap::value_parser!(i64).range(0..))]
    pub download_limit: Option<i64>,
}

/// Create a share for one file or folder.
#[derive(Debug, Args)]
pub struct CreateShareArgs {
    /// ADrive file or folder target
    #[arg(conflicts_with_all = ["instance", "space", "folder", "file"])]
    pub target: Option<String>,
    /// ADrive instance identifier
    #[arg(long)]
    pub instance: Option<String>,
    /// ADrive space identifier
    #[arg(long)]
    pub space: Option<String>,
    /// Folder path inside the space
    #[arg(long)]
    pub folder: Option<String>,
    /// File name inside the folder
    #[arg(long)]
    pub file: Option<String>,
    /// Treat ADrive instance/space target segments as names and resolve them to IDs
    #[arg(long = "by-name")]
    pub is_by_name: bool,
    /// Share page origin override
    // [Review Fix #ShareEndpoint2] Keep existing invocations valid without advertising the former option name.
    #[arg(long, alias = "share-base-url")]
    pub share_endpoint: Option<String>,
    #[command(flatten)]
    pub settings: ShareSettings,
}

/// Existing share selector.
#[derive(Debug, Args)]
pub struct ShareIdArgs {
    /// Share identifier or adrive://instance/share-id
    pub share_id: String,
    /// ADrive instance identifier; required for a bare ShareID, optional for a share URI
    #[arg(long)]
    pub instance: Option<String>,
    /// Treat the share target instance as a name and resolve it to an ID
    #[arg(long = "by-name")]
    pub is_by_name: bool,
    /// Share page origin override
    #[arg(long, alias = "share-base-url")]
    pub share_endpoint: Option<String>,
}

/// Cancellation selector; revocation requires no share-page origin.
#[derive(Debug, Args)]
pub struct CancelShareArgs {
    /// Share identifier or adrive://instance/share-id
    pub share_id: String,
    /// ADrive instance identifier; required for a bare ShareID, optional for a share URI
    #[arg(long)]
    pub instance: Option<String>,
    /// Treat the share target instance as a name and resolve it to an ID
    #[arg(long = "by-name")]
    pub is_by_name: bool,
}

/// Search one page of share links.
#[derive(Debug, Args)]
pub struct ListShareArgs {
    /// ADrive instance target: adrive://instance
    pub target: Option<String>,
    /// ADrive instance identifier; required unless an instance URI is supplied
    #[arg(long)]
    pub instance: Option<String>,
    /// Treat the share target instance as a name and resolve it to an ID
    #[arg(long = "by-name")]
    pub is_by_name: bool,
    /// Creator user identifier; repeat for multiple creators
    #[arg(long)]
    pub creator: Vec<String>,
    /// Share lifecycle status
    #[arg(long, value_enum)]
    pub status: Option<ShareStatus>,
    /// Share name
    #[arg(long)]
    pub name: Option<String>,
    /// Creation time range start in RFC3339
    #[arg(long)]
    pub created_at_start: Option<String>,
    /// Creation time range end in RFC3339
    #[arg(long)]
    pub created_at_end: Option<String>,
    /// Search offset, at most 10000
    #[arg(long, value_parser = clap::value_parser!(i64).range(0..=10000))]
    pub offset: Option<i64>,
    /// Maximum entries in this page
    #[arg(long, value_parser = clap::value_parser!(i64).range(1..))]
    pub limit: Option<i64>,
    /// Share page origin override
    #[arg(long, alias = "share-base-url")]
    pub share_endpoint: Option<String>,
}

/// Update one share with an optional optimistic concurrency guard.
#[derive(Debug, Args)]
pub struct UpdateShareArgs {
    #[command(flatten)]
    pub selector: ShareIdArgs,
    /// Optimistic concurrency timestamp; omitted fetches the current value
    #[arg(long)]
    pub updated_at: Option<i64>,
    #[command(flatten)]
    pub settings: ShareSettings,
}

/// Public share page selector.
#[derive(Args)]
pub struct InspectShareArgs {
    /// Public share URL (one optional trailing slash); optional pwd query is kept secret
    pub share_url: String,
}

/// Receiver file selection with a combined current-level result cap.
#[derive(Args)]
pub struct ShareFilesArgs {
    /// Public share URL (one optional trailing slash); optional pwd query is kept secret
    pub share_url: String,
    /// Exact file path or directory path ending in slash
    #[arg(long)]
    pub path: Option<String>,
    /// Pagination marker returned by a previous listing
    #[arg(long)]
    pub marker: Option<String>,
    /// Maximum entries to return from the current directory level
    #[arg(long, default_value = "1000", value_parser = clap::value_parser!(i32).range(1..))]
    pub max_keys: i32,
    /// Read the receiver password from stdin; it does not change the share password (EOF; use a pipe or Enter then Ctrl-D on macOS/Linux)
    #[arg(long = "password-stdin")]
    pub is_password_stdin: bool,
}

/// Preview selector; raw output requires a pipe or redirected stdout.
#[derive(Args)]
pub struct ShareViewArgs {
    /// Public share URL (one optional trailing slash); optional pwd query is kept secret
    pub share_url: String,
    /// Exact file path; omitted selects a single shared file
    #[arg(long)]
    pub path: Option<String>,
    /// Stream original bytes to a pipe or redirected stdout
    #[arg(long = "raw")]
    pub is_raw: bool,
    /// Read the receiver password from stdin; it does not change the share password (EOF; use a pipe or Enter then Ctrl-D on macOS/Linux)
    #[arg(long = "password-stdin")]
    pub is_password_stdin: bool,
}

impl std::fmt::Debug for ShareViewArgs {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let safe_url = crate::domain::share_url::parse_share_url(&self.share_url)
            .map(|parsed| parsed.safe_url)
            .unwrap_or_else(|_| "[invalid share URL]".into());
        formatter
            .debug_struct("ShareViewArgs")
            .field("share_url", &safe_url)
            .field("is_raw", &self.is_raw)
            .finish_non_exhaustive()
    }
}

/// Download selection; destination always names a directory.
#[derive(Args)]
pub struct ShareDownloadArgs {
    /// Public share URL (one optional trailing slash); optional pwd query is kept secret
    pub share_url: String,
    /// Destination directory for shared files
    pub destination_dir: String,
    /// Exact file path or directory path ending in slash
    #[arg(long)]
    pub path: Option<String>,
    /// Recursively download selected folders
    #[arg(long = "recursive")]
    pub is_recursive: bool,
    /// Overwrite existing destination files
    #[arg(long = "overwrite")]
    pub is_overwrite: bool,
    /// Read the receiver password from stdin; it does not change the share password (EOF; use a pipe or Enter then Ctrl-D on macOS/Linux)
    #[arg(long = "password-stdin")]
    pub is_password_stdin: bool,
    /// Checkpoint file for file-level resume
    #[arg(long)]
    pub checkpoint: Option<String>,
    /// Automatically range-download files at this size (default 20MB), even without a checkpoint
    #[arg(long)]
    pub checkpoint_threshold: Option<String>,
    /// Maximum ranges transferred concurrently for one file (default 4)
    #[arg(long)]
    pub multipart_concurrency: Option<usize>,
    /// Resume using the checkpoint file
    #[arg(long = "resume", requires = "checkpoint")]
    pub is_resume: bool,
    /// Enable execution progress output even when stderr is not a TTY
    #[arg(long = "progress", conflicts_with = "is_no_progress")]
    pub is_progress: bool,
    /// Disable execution progress output
    #[arg(long = "no-progress", conflicts_with = "is_progress")]
    pub is_no_progress: bool,
}

// [Review Fix #ShareSecret2] Debug must never echo URL query passwords.
impl std::fmt::Debug for InspectShareArgs {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let safe_url = crate::domain::share_url::parse_share_url(&self.share_url)
            .map(|parsed| parsed.safe_url)
            .unwrap_or_else(|_| "[invalid share URL]".into());
        formatter
            .debug_struct("InspectShareArgs")
            .field("share_url", &safe_url)
            .finish_non_exhaustive()
    }
}

// [Review Fix #ShareSecret2] Debug must never echo URL query passwords.
impl std::fmt::Debug for ShareFilesArgs {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let safe_url = crate::domain::share_url::parse_share_url(&self.share_url)
            .map(|parsed| parsed.safe_url)
            .unwrap_or_else(|_| "[invalid share URL]".into());
        formatter
            .debug_struct("ShareFilesArgs")
            .field("share_url", &safe_url)
            .finish_non_exhaustive()
    }
}

// [Review Fix #ShareSecret2] Debug must never echo URL query passwords.
impl std::fmt::Debug for ShareDownloadArgs {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let safe_url = crate::domain::share_url::parse_share_url(&self.share_url)
            .map(|parsed| parsed.safe_url)
            .unwrap_or_else(|_| "[invalid share URL]".into());
        formatter
            .debug_struct("ShareDownloadArgs")
            .field("share_url", &safe_url)
            .finish_non_exhaustive()
    }
}

#[cfg(test)]
mod files_pagination_tests {
    use super::*;

    #[test]
    fn files_accepts_max_keys_and_rejects_removed_limit() {
        let command = ShareFilesArgs::augment_args(clap::Command::new("files"));
        let parsed = command.clone().try_get_matches_from([
            "files",
            "https://example.test/share/s",
            "--max-keys",
            "4",
        ]);
        assert!(
            parsed.is_ok(),
            "--max-keys must select the result cap: {parsed:?}"
        );
        assert!(
            command
                .try_get_matches_from(["files", "https://example.test/share/s", "--limit", "4",])
                .is_err(),
            "removed --limit must not remain an alias"
        );
    }
}
