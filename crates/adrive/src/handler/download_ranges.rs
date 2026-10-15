//! Transport-independent checked byte ranges and bounded concurrent scheduling.
use futures::{stream, StreamExt, TryStreamExt};
use std::future::Future;
use tos_core::agent::error::CliError;

pub(super) const DOWNLOAD_PART_SIZE: u64 = 16 * 1024 * 1024;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct DownloadRange {
    pub number: u32,
    pub offset: u64,
    pub size: u64,
}
impl DownloadRange {
    /// Return the inclusive Range header for this nonempty checked range.
    pub fn header(self) -> String {
        format!("bytes={}-{}", self.offset, self.offset + self.size - 1)
    }
    /// Validate a range response before any body is written; never include remote header contents in errors.
    pub fn validate(
        self,
        status: u16,
        content_range: Option<&str>,
        total: u64,
    ) -> Result<(), CliError> {
        let expected = format!(
            "bytes {}-{}/{}",
            self.offset,
            self.offset + self.size - 1,
            total
        );
        if status != 206 || content_range.map(str::trim) != Some(expected.as_str()) {
            return Err(CliError::TransferFailed(
                "range response must return HTTP 206 and the exact Content-Range".into(),
            ));
        }
        Ok(())
    }
}

#[derive(Clone)]
pub(super) struct RangePlan {
    size: u64,
    offset: u64,
    next_number: u32,
}
impl RangePlan {
    /// Lazily plan fixed-size ranges; reject more parts than the checkpoint numbering format can represent.
    pub fn new(size: u64) -> Result<Self, CliError> {
        if size.div_ceil(DOWNLOAD_PART_SIZE) > u32::MAX as u64 {
            return Err(CliError::ValidationError(
                "file exceeds range part-count limit".into(),
            ));
        }
        Ok(Self {
            size,
            offset: 0,
            next_number: 1,
        })
    }
}
impl Iterator for RangePlan {
    type Item = DownloadRange;
    fn next(&mut self) -> Option<Self::Item> {
        if self.offset == self.size {
            return None;
        }
        let size = (self.size - self.offset).min(DOWNLOAD_PART_SIZE);
        let range = DownloadRange {
            number: self.next_number,
            offset: self.offset,
            size,
        };
        self.offset += size;
        self.next_number = self.next_number.saturating_add(1);
        Some(range)
    }
}

/// Execute range jobs with bounded concurrency; return the first failure and cancel remaining jobs.
pub(super) async fn run_ranges<I, F, R>(
    ranges: I,
    concurrency: usize,
    transfer: F,
) -> Result<(), CliError>
where
    I: IntoIterator<Item = DownloadRange>,
    F: FnMut(DownloadRange) -> R,
    R: Future<Output = Result<(), CliError>>,
{
    if concurrency == 0 {
        return Err(CliError::ValidationError(
            "multipart concurrency must be positive".into(),
        ));
    }
    stream::iter(ranges)
        .map(Ok)
        .try_for_each_concurrent(concurrency, transfer)
        .await
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn range_boundaries_are_exact_and_overflow_is_rejected() {
        assert_eq!(RangePlan::new(0).unwrap().count(), 0);
        let ranges: Vec<_> = RangePlan::new(DOWNLOAD_PART_SIZE + 1).unwrap().collect();
        assert_eq!(ranges[0].header(), "bytes=0-16777215");
        assert_eq!(ranges[1].header(), "bytes=16777216-16777216");
        assert_eq!(RangePlan::new(DOWNLOAD_PART_SIZE).unwrap().count(), 1);
        assert!(RangePlan::new(u64::MAX).is_err());
        assert!(ranges[1]
            .validate(
                206,
                Some("bytes 16777216-16777216/16777217"),
                DOWNLOAD_PART_SIZE + 1
            )
            .is_ok());
        assert!(ranges[1]
            .validate(200, None, DOWNLOAD_PART_SIZE + 1)
            .is_err());
        assert!(ranges[1]
            .validate(206, Some("bytes 0-0/1"), DOWNLOAD_PART_SIZE + 1)
            .is_err());
    }
    #[tokio::test]
    async fn scheduler_bounds_concurrency_and_stops_on_failure() {
        use std::sync::atomic::{AtomicUsize, Ordering};
        let active = AtomicUsize::new(0);
        let peak = AtomicUsize::new(0);
        run_ranges(
            RangePlan::new(DOWNLOAD_PART_SIZE * 5).unwrap(),
            2,
            |_| async {
                let count = active.fetch_add(1, Ordering::SeqCst) + 1;
                peak.fetch_max(count, Ordering::SeqCst);
                tokio::task::yield_now().await;
                active.fetch_sub(1, Ordering::SeqCst);
                Ok(())
            },
        )
        .await
        .unwrap();
        assert_eq!(peak.load(Ordering::SeqCst), 2);
        let started = AtomicUsize::new(0);
        let result = run_ranges(
            RangePlan::new(DOWNLOAD_PART_SIZE * 5).unwrap(),
            1,
            |_| async {
                started.fetch_add(1, Ordering::SeqCst);
                Err(CliError::TransferFailed("fixture failure".into()))
            },
        )
        .await;
        assert!(result.is_err());
        assert_eq!(started.load(Ordering::SeqCst), 1);
        assert!(
            run_ranges(RangePlan::new(1).unwrap(), 0, |_| async { Ok(()) })
                .await
                .is_err()
        );
    }
}
