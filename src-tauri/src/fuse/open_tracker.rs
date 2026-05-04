use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Duration, Instant};
use parking_lot::Mutex;
use serde::{Deserialize, Serialize};

const DIALOG_TIMEOUT: Duration = Duration::from_secs(300);
const BATCH_WINDOW: Duration = Duration::from_millis(500);

macro_rules! ot_log {
    ($($arg:tt)*) => {
        println!("💬 [OPEN-TRK] {}", format!($($arg)*))
    };
}

macro_rules! ot_warn {
    ($($arg:tt)*) => {
        eprintln!("⚠️  [OPEN-TRK] {}", format!($($arg)*))
    };
}

macro_rules! ot_step {
    ($step:expr, $($arg:tt)*) => {
        println!("👉 [OPEN-TRK::{}] {}", $step, format!($($arg)*))
    };
}

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
        ot_log!("Open tracker created");
        Self {
            pending: Mutex::new(HashMap::new()),
            dialog_queue: Mutex::new(Vec::new()),
            app_handle: Mutex::new(None),
        }
    }

    pub fn set_app_handle(&self, handle: tauri::AppHandle) {
        ot_step!("SET_HANDLE", "Setting Tauri app handle for dialog events");
        *self.app_handle.lock() = Some(handle);
    }

    pub fn request_open(&self, file_info: FileInfo) -> Option<PathBuf> {
        let file_id = file_info.file_id.clone();
        let file_name = file_info.file_name.clone();
        ot_step!("REQUEST_OPEN", "File '{}' (id='{}', size={}) needs download dialog", 
                 file_name, file_id, file_info.file_size);

        let (tx, rx) = crossbeam_channel::bounded(1);

        let pending = PendingOpen {
            file_info,
            timestamp: Instant::now(),
            response_tx: tx,
        };

        {
            let mut pending_map = self.pending.lock();
            ot_log!("Adding to pending map: '{}' (total pending: {})", file_id, pending_map.len() + 1);
            pending_map.insert(file_id.clone(), pending);
        }

        ot_step!("EMIT_DIALOG", "Emitting dialog event to frontend for '{}'", file_name);
        self.check_and_emit_dialog();

        ot_log!("Waiting for user response (timeout: {}s)...", DIALOG_TIMEOUT.as_secs());
        match rx.recv_timeout(DIALOG_TIMEOUT) {
            Ok(Some(path)) => {
                ot_step!("GOT_RESPONSE", "User chose path for '{}': {}", file_name, path.display());
                Some(path)
            }
            Ok(None) => {
                ot_warn!("User cancelled dialog for '{}'", file_name);
                self.pending.lock().remove(&file_id);
                None
            }
            Err(_) => {
                ot_warn!("Dialog TIMEOUT for '{}' ({}s)", file_name, DIALOG_TIMEOUT.as_secs());
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

            for (id, pending) in pending_map.iter() {
                ot_log!("Pending file: '{}' age={}ms", id, now.duration_since(pending.timestamp).as_millis());
                if now.duration_since(pending.timestamp) < BATCH_WINDOW || batch.is_empty() {
                    batch.push(pending.file_info.clone());
                }
            }

            ot_log!("Batched {} file(s) for dialog", batch.len());
            batch
        };

        if batch.is_empty() {
            ot_log!("No files to show in dialog");
            return;
        }

        let handle = self.app_handle.lock();
        if let Some(ref app_handle) = *handle {
            ot_step!("EMIT", "Emitting 'fuse-download-request' event with {} file(s)", batch.len());
            match tauri::Manager::emit_all(app_handle, "fuse-download-request", &batch) {
                Ok(_) => ot_log!("Event emitted successfully"),
                Err(e) => ot_warn!("Failed to emit event: {}", e),
            }
        } else {
            ot_warn!("No app handle set! Cannot emit dialog event. File opens will timeout.");
        }
    }

    pub fn respond(&self, result: DialogResult) {
        ot_step!("RESPOND", "Received dialog response: {:?}", match &result {
            DialogResult::SaveTo { file_id, path } => format!("SaveTo('{}' -> '{}')", file_id, path),
            DialogResult::SaveAllTo { path } => format!("SaveAllTo('{}')", path),
            DialogResult::Cancel { file_id } => format!("Cancel('{}')", file_id),
            DialogResult::CancelAll => "CancelAll".to_string(),
        });

        match result {
            DialogResult::SaveTo { file_id, path } => {
                let mut pending_map = self.pending.lock();
                if let Some(pending) = pending_map.remove(&file_id) {
                    ot_log!("Sending SaveTo response for '{}'", file_id);
                    let _ = pending.response_tx.send(Some(PathBuf::from(&path)));
                } else {
                    ot_warn!("No pending open found for file_id='{}'", file_id);
                }
            }
            DialogResult::SaveAllTo { path } => {
                let mut pending_map = self.pending.lock();
                let count = pending_map.len();
                let dest = PathBuf::from(&path);
                ot_log!("Sending SaveAllTo response for {} pending file(s)", count);
                for (id, pending) in pending_map.drain() {
                    let file_dest = dest.join(&pending.file_info.file_name);
                    ot_log!("  -> '{}' will save to '{}'", id, file_dest.display());
                    let _ = pending.response_tx.send(Some(file_dest));
                }
            }
            DialogResult::Cancel { file_id } => {
                let mut pending_map = self.pending.lock();
                if let Some(pending) = pending_map.remove(&file_id) {
                    ot_log!("Sending Cancel response for '{}'", file_id);
                    let _ = pending.response_tx.send(None);
                } else {
                    ot_warn!("No pending open found for cancelled file_id='{}'", file_id);
                }
            }
            DialogResult::CancelAll => {
                let mut pending_map = self.pending.lock();
                let count = pending_map.len();
                ot_log!("Sending CancelAll response for {} pending file(s)", count);
                for (id, pending) in pending_map.drain() {
                    ot_log!("  -> Cancelling '{}'", id);
                    let _ = pending.response_tx.send(None);
                }
            }
        }
    }

    pub fn cancel_all(&self) {
        ot_step!("CANCEL_ALL", "Cancelling all pending opens...");
        let mut pending_map = self.pending.lock();
        let count = pending_map.len();
        for (id, pending) in pending_map.drain() {
            ot_log!("  -> Cancelled '{}'", id);
            let _ = pending.response_tx.send(None);
        }
        ot_log!("Cancelled {} pending open(s)", count);
    }
}

impl Default for OpenTracker {
    fn default() -> Self {
        Self::new()
    }
}
