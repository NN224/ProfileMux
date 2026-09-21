use std::path::PathBuf;
use std::sync::mpsc::{channel, Receiver, Sender};
use std::thread;

use crate::domain::{ProfileId, StorageBreakdown};

/// Work item for background profile storage measurement.
#[derive(Debug, Clone)]
pub struct ScanRequest {
    pub profile_id: ProfileId,
    pub profile_path: PathBuf,
    pub cache_path: Option<PathBuf>,
}

/// Asynchronous background worker for measuring profile disk usage without
/// blocking the TUI event loop.
pub struct SizeScanner {
    tx_request: Sender<ScanRequest>,
    rx_result: Receiver<(ProfileId, StorageBreakdown)>,
}

impl SizeScanner {
    /// Spawns a single background worker thread draining measurement requests.
    pub fn new() -> Self {
        let (tx_request, rx_request) = channel::<ScanRequest>();
        let (tx_result, rx_result) = channel::<(ProfileId, StorageBreakdown)>();

        let _ = thread::Builder::new()
            .name("pmux-size-worker".to_string())
            .spawn(move || {
                while let Ok(req) = rx_request.recv() {
                    let breakdown = crate::fs::size::measure_profile(
                        &req.profile_path,
                        req.cache_path.as_deref(),
                    );
                    if tx_result.send((req.profile_id, breakdown)).is_err() {
                        break;
                    }
                }
            });

        Self {
            tx_request,
            rx_result,
        }
    }

    /// Submits a profile to be measured in the background.
    pub fn request_scan(&self, request: ScanRequest) {
        let _ = self.tx_request.send(request);
    }

    /// Drains all completed size measurements accumulated since the last check.
    pub fn drain_results(&self) -> Vec<(ProfileId, StorageBreakdown)> {
        let mut results = Vec::new();
        while let Ok(res) = self.rx_result.try_recv() {
            results.push(res);
        }
        results
    }
}

impl Default for SizeScanner {
    fn default() -> Self {
        Self::new()
    }
}
