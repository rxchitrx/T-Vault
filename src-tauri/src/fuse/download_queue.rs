use std::collections::VecDeque;
use std::path::PathBuf;
use std::sync::Arc;
use parking_lot::Mutex;
use grammers_client::Client;
use tokio::sync::Mutex as AsyncMutex;
use anyhow::Result;
use std::time::Duration;

const MAX_RETRIES: u32 = 3;
const RETRY_DELAY: Duration = Duration::from_secs(2);

macro_rules! dl_log {
    ($($arg:tt)*) => {
        println!("📥 [DL-Q] {}", format!($($arg)*))
    };
}

macro_rules! dl_warn {
    ($($arg:tt)*) => {
        eprintln!("⚠️  [DL-Q] {}", format!($($arg)*))
    };
}

macro_rules! dl_step {
    ($step:expr, $($arg:tt)*) => {
        println!("👉 [DL-Q::{}] {}", $step, format!($($arg)*))
    };
}

#[derive(Debug, Clone, PartialEq)]
pub enum DownloadStatus {
    Pending,
    Downloading { progress: u8 },
    Completed(PathBuf),
    Failed(String),
    Cancelled,
}

#[derive(Debug, Clone)]
pub struct DownloadTask {
    pub file_id: String,
    pub file_name: String,
    pub file_size: u64,
    pub destination: PathBuf,
    pub status: DownloadStatus,
    pub retry_count: u32,
}

pub struct DownloadQueue {
    queue: Mutex<VecDeque<DownloadTask>>,
    current: Mutex<Option<DownloadTask>>,
    client_ref: Arc<AsyncMutex<Option<Client>>>,
}

impl DownloadQueue {
    pub fn new(client_ref: Arc<AsyncMutex<Option<Client>>>) -> Self {
        dl_log!("Download queue created");
        Self {
            queue: Mutex::new(VecDeque::new()),
            current: Mutex::new(None),
            client_ref,
        }
    }

    pub fn enqueue(&self, file_id: String, file_name: String, file_size: u64, destination: PathBuf) {
        dl_step!("ENQUEUE", "file='{}' id='{}' size={} dest='{}'", 
                 file_name, file_id, file_size, destination.display());
        println!("T-Vault FUSE: Queued download for '{}' ({})", file_name, file_id);
        let task = DownloadTask {
            file_id,
            file_name,
            file_size,
            destination,
            status: DownloadStatus::Pending,
            retry_count: 0,
        };
        let queue_len = {
            let mut queue = self.queue.lock();
            queue.push_back(task);
            queue.len()
        };
        dl_log!("Queue now has {} task(s)", queue_len);
    }

    pub fn get_status(&self, file_id: &str) -> DownloadStatus {
        {
            let current = self.current.lock();
            if let Some(ref task) = *current {
                if task.file_id == file_id {
                    dl_log!("get_status('{}') -> current task: {:?}", file_id, task.status);
                    return task.status.clone();
                }
            }
        }
        
        let queue = self.queue.lock();
        for task in queue.iter() {
            if task.file_id == file_id {
                dl_log!("get_status('{}') -> queued task: Pending", file_id);
                return task.status.clone();
            }
        }
        
        dl_warn!("get_status('{}') -> not found in queue or current", file_id);
        DownloadStatus::Failed("Not found in queue".to_string())
    }

    pub fn has_pending(&self, file_id: &str) -> bool {
        {
            let current = self.current.lock();
            if let Some(ref task) = *current {
                if task.file_id == file_id {
                    dl_log!("has_pending('{}') -> yes (current task)", file_id);
                    return true;
                }
            }
        }
        
        let queue = self.queue.lock();
        let found = queue.iter().any(|t| t.file_id == file_id);
        dl_log!("has_pending('{}') -> {} (in queue)", file_id, found);
        found
    }

    pub fn cancel_all(&self) {
        dl_step!("CANCEL_ALL", "Cancelling all downloads...");
        {
            let mut current = self.current.lock();
            if let Some(ref mut task) = *current {
                task.status = DownloadStatus::Cancelled;
                dl_log!("Cancelled current task: '{}'", task.file_name);
            }
        }
        
        let mut queue = self.queue.lock();
        let count = queue.len();
        for task in queue.iter_mut() {
            task.status = DownloadStatus::Cancelled;
        }
        queue.clear();
        dl_log!("Cancelled and cleared {} queued task(s)", count);
    }

    pub fn get_all_tasks(&self) -> Vec<DownloadTask> {
        let mut tasks = Vec::new();
        
        {
            let current = self.current.lock();
            if let Some(ref task) = *current {
                tasks.push(task.clone());
            }
        }
        
        let queue = self.queue.lock();
        for task in queue.iter() {
            tasks.push(task.clone());
        }
        
        tasks
    }

    pub async fn process_next(&self) -> Result<Option<DownloadTask>> {
        let task = {
            let mut queue = self.queue.lock();
            let t = queue.pop_front();
            if let Some(ref t) = t {
                dl_step!("PROCESS_NEXT", "Picked task: '{}' from queue", t.file_name);
            } else {
                dl_log!("process_next: queue is empty");
            }
            t
        };

        let task = match task {
            Some(t) => t,
            None => return Ok(None),
        };

        dl_log!("Starting download for '{}' (id='{}') to '{}'", 
                task.file_name, task.file_id, task.destination.display());

        {
            let mut current = self.current.lock();
            *current = Some(task.clone());
        }

        let result = self.download_with_retry(&task).await;

        match result {
            Ok(path) => {
                dl_log!("Download SUCCESS: '{}' -> {}", task.file_name, path.display());
                let mut current = self.current.lock();
                if let Some(ref mut t) = *current {
                    t.status = DownloadStatus::Completed(path);
                }
                Ok(current.clone())
            }
            Err(e) => {
                dl_warn!("Download FAILED: '{}' - {}", task.file_name, e);
                let mut current = self.current.lock();
                if let Some(ref mut t) = *current {
                    t.status = DownloadStatus::Failed(e.to_string());
                }
                let failed_task = current.clone();
                *current = None;
                Err(e)
            }
        }
    }

    async fn download_with_retry(&self, task: &DownloadTask) -> Result<PathBuf> {
        let mut attempt = 0;
        let mut last_error = String::new();

        while attempt < MAX_RETRIES {
            if attempt > 0 {
                dl_step!("RETRY", "Retrying '{}' (attempt {}/{}) after {}s delay", 
                         task.file_name, attempt + 1, MAX_RETRIES, RETRY_DELAY.as_secs());
                tokio::time::sleep(RETRY_DELAY).await;
            }

            {
                let mut current = self.current.lock();
                if let Some(ref mut t) = *current {
                    t.status = DownloadStatus::Downloading { progress: 0 };
                    dl_step!("DOWNLOAD_START", "Attempt {} for '{}' ({} bytes)", 
                             attempt + 1, task.file_name, task.file_size);
                }
            }

            match self.download_file(task).await {
                Ok(path) => {
                    dl_log!("Download attempt {} SUCCEEDED: '{}'", attempt + 1, task.file_name);
                    return Ok(path);
                }
                Err(e) => {
                    last_error = e.to_string();
                    dl_warn!("Download attempt {} FAILED for '{}': {}", attempt + 1, task.file_name, last_error);
                    
                    let mut current = self.current.lock();
                    if let Some(ref mut t) = *current {
                        if matches!(t.status, DownloadStatus::Cancelled) {
                            dl_log!("Download was cancelled, stopping retries");
                            return Err(anyhow::anyhow!("Download cancelled"));
                        }
                        t.retry_count = attempt + 1;
                    }
                }
            }

            attempt += 1;
        }

        dl_warn!("All {} retries exhausted for '{}'", MAX_RETRIES, task.file_name);
        Err(anyhow::anyhow!("Download failed after {} retries: {}", MAX_RETRIES, last_error))
    }

    async fn download_file(&self, task: &DownloadTask) -> Result<PathBuf> {
        let file_id = task.file_id.clone();
        let destination = task.destination.to_string_lossy().to_string();
        let client_ref = self.client_ref.clone();
        let file_size = task.file_size;

        dl_step!("TELEGRAM_DOWNLOAD", "Downloading '{}' ({} bytes) from Telegram to '{}'", 
                 task.file_name, file_size, destination);

        crate::storage::download_file(
            client_ref,
            &file_id,
            &destination,
            move |_, downloaded, total| {
                if total > 0 {
                    let progress = (downloaded as f64 / total as f64 * 100.0) as u8;
                    dl_log!("Progress: {}/{} bytes ({}%)", downloaded, total, progress);
                }
            },
        ).await?;

        let path = PathBuf::from(&destination);
        if path.exists() {
            let actual_size = std::fs::metadata(&path).map(|m| m.len()).unwrap_or(0);
            dl_log!("Download file verified: '{}' exists, {} bytes on disk", task.file_name, actual_size);
            Ok(path)
        } else {
            dl_warn!("Download reported success but file not found at '{}'", destination);
            Err(anyhow::anyhow!("Download completed but file not found at {}", destination))
        }
    }

    pub fn check_disk_space(&self, required_bytes: u64, destination: &PathBuf) -> Result<()> {
        if let Some(parent) = destination.parent() {
            let available = fs_available_space(parent);
            dl_step!("DISK_CHECK", "Required: {} bytes, Available: {} bytes at '{}'", 
                     required_bytes, available, parent.display());
            if available < required_bytes {
                dl_warn!("INSUFFICIENT DISK SPACE: need {} bytes, have {} bytes", required_bytes, available);
                return Err(anyhow::anyhow!(
                    "Not enough disk space. Required: {} bytes, Available: {} bytes",
                    required_bytes, available
                ));
            }
            dl_log!("Disk space OK");
        }
        Ok(())
    }

    pub fn cleanup_partial(&self, path: &PathBuf) {
        dl_step!("CLEANUP", "Removing partial file at '{}'", path.display());
        if path.exists() {
            if let Err(e) = std::fs::remove_file(path) {
                dl_warn!("Failed to cleanup partial file: {}", e);
            } else {
                dl_log!("Partial file cleaned up successfully");
            }
        } else {
            dl_log!("No partial file to clean up (already gone)");
        }
    }
}

fn fs_available_space(path: &std::path::Path) -> u64 {
    #[cfg(target_os = "macos")]
    {
        use std::ffi::CString;
        use libc::statfs;
        
        let c_path = match CString::new(path.to_string_lossy().into_owned()) {
            Ok(p) => p,
            Err(_) => return 0,
        };
        
        unsafe {
            let mut buf: statfs = std::mem::zeroed();
            if statfs(c_path.as_ptr(), &mut buf) == 0 {
                return buf.f_bavail as u64 * buf.f_bsize as u64;
            }
        }
        0
    }
    
    #[cfg(not(target_os = "macos"))]
    {
        let _ = path;
        u64::MAX
    }
}
