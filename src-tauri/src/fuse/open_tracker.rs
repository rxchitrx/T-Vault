use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Duration, Instant};
use parking_lot::Mutex;
use serde::{Deserialize, Serialize};

const DIALOG_TIMEOUT: Duration = Duration::from_secs(300);
const BATCH_WINDOW: Duration = Duration::from_millis(500);

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileInfo {
    pub file_id: String,
    pub file_name: String,
    pub file_size: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum DialogResult {
    SaveTo { file_id: String, path: String },
    SaveAllTo { path: String },
    Cancel { file_id: String },
    CancelAll,
}

struct PendingOpen {
    file_info: FileInfo,
    timestamp: Instant,
    response_tx: crossbeam_channel::Sender<Option<PathBuf>>,
}

pub struct OpenTracker {
    pending: Mutex<HashMap<String, PendingOpen>>,
    dialog_queue: Mutex<Vec<Vec<FileInfo>>>,
    app_handle: Mutex<Option<tauri::AppHandle>>,
}

impl OpenTracker {
    pub fn new() -> Self {
        Self {
            pending: Mutex::new(HashMap::new()),
            dialog_queue: Mutex::new(Vec::new()),
            app_handle: Mutex::new(None),
        }
    }

    pub fn set_app_handle(&self, handle: tauri::AppHandle) {
        *self.app_handle.lock() = Some(handle);
    }

    pub fn request_open(&self, file_info: FileInfo) -> Option<PathBuf> {
        let file_id = file_info.file_id.clone();
        let (tx, rx) = crossbeam_channel::bounded(1);

        let pending = PendingOpen {
            file_info,
            timestamp: Instant::now(),
            response_tx: tx,
        };

        {
            let mut pending_map = self.pending.lock();
            pending_map.insert(file_id.clone(), pending);
        }

        self.check_and_emit_dialog();

        match rx.recv_timeout(DIALOG_TIMEOUT) {
            Ok(Some(path)) => Some(path),
            Ok(None) => {
                println!("T-Vault FUSE: User cancelled open for '{}'", file_id);
                None
            }
            Err(_) => {
                println!("T-Vault FUSE: Dialog timeout for '{}'", file_id);
                self.pending.lock().remove(&file_id);
                None
            }
        }
    }

    fn check_and_emit_dialog(&self) {
        let batch = {
            let mut pending_map = self.pending.lock();
            let now = Instant::now();
            
            let mut batch: Vec<FileInfo> = Vec::new();
            let mut to_remove: Vec<String> = Vec::new();

            for (id, pending) in pending_map.iter() {
                if now.duration_since(pending.timestamp) < BATCH_WINDOW || batch.is_empty() {
                    batch.push(pending.file_info.clone());
                    if now.duration_since(pending.timestamp) >= BATCH_WINDOW {
                        to_remove.push(id.clone());
                    }
                }
            }

            for id in &to_remove {
                pending_map.remove(id);
            }

            batch
        };

        if batch.is_empty() {
            return;
        }

        let handle = self.app_handle.lock();
        if let Some(ref app_handle) = *handle {
            let _ = tauri::Manager::emit_all(app_handle, "fuse-download-request", &batch);
            println!("T-Vault FUSE: Emitted download dialog for {} file(s)", batch.len());
        }
    }

    pub fn respond(&self, result: DialogResult) {
        match result {
            DialogResult::SaveTo { file_id, path } => {
                let mut pending_map = self.pending.lock();
                if let Some(pending) = pending_map.remove(&file_id) {
                    let _ = pending.response_tx.send(Some(PathBuf::from(&path)));
                }
            }
            DialogResult::SaveAllTo { path } => {
                let mut pending_map = self.pending.lock();
                let dest = PathBuf::from(&path);
                for (_, pending) in pending_map.drain() {
                    let file_dest = dest.join(&pending.file_info.file_name);
                    let _ = pending.response_tx.send(Some(file_dest));
                }
            }
            DialogResult::Cancel { file_id } => {
                let mut pending_map = self.pending.lock();
                if let Some(pending) = pending_map.remove(&file_id) {
                    let _ = pending.response_tx.send(None);
                }
            }
            DialogResult::CancelAll => {
                let mut pending_map = self.pending.lock();
                for (_, pending) in pending_map.drain() {
                    let _ = pending.response_tx.send(None);
                }
            }
        }
    }

    pub fn cancel_all(&self) {
        let mut pending_map = self.pending.lock();
        for (_, pending) in pending_map.drain() {
            let _ = pending.response_tx.send(None);
        }
    }
}

impl Default for OpenTracker {
    fn default() -> Self {
        Self::new()
    }
}
