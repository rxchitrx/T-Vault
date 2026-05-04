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
        Self {
            queue: Mutex::new(VecDeque::new()),
            current: Mutex::new(None),
            client_ref,
        }
    }

    pub fn enqueue(&self, file_id: String, file_name: String, file_size: u64, destination: PathBuf) {
        println!("T-Vault FUSE: Queued download for '{}' ({})", file_name, file_id);
        let task = DownloadTask {
            file_id,
            file_name,
            file_size,
            destination,
            status: DownloadStatus::Pending,
            retry_count: 0,
        };
        self.queue.lock().push_back(task);
    }

    pub fn get_status(&self, file_id: &str) -> DownloadStatus {
        {
            let current = self.current.lock();
            if let Some(ref task) = *current {
                if task.file_id == file_id {
                    return task.status.clone();
                }
            }
        }
        
        let queue = self.queue.lock();
        for task in queue.iter() {
            if task.file_id == file_id {
                return task.status.clone();
            }
        }
        
        DownloadStatus::Failed("Not found in queue".to_string())
    }

    pub fn has_pending(&self, file_id: &str) -> bool {
        {
            let current = self.current.lock();
            if let Some(ref task) = *current {
                if task.file_id == file_id {
                    return true;
                }
            }
        }
        
        let queue = self.queue.lock();
        queue.iter().any(|t| t.file_id == file_id)
    }

    pub fn cancel_all(&self) {
        {
            let mut current = self.current.lock();
            if let Some(ref mut task) = *current {
                task.status = DownloadStatus::Cancelled;
            }
        }
        
        let mut queue = self.queue.lock();
        for task in queue.iter_mut() {
            task.status = DownloadStatus::Cancelled;
        }
        queue.clear();
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
            queue.pop_front()
        };

        let task = match task {
            Some(t) => t,
            None => return Ok(None),
        };

        {
            let mut current = self.current.lock();
            *current = Some(task.clone());
        }

        let result = self.download_with_retry(&task).await;

        match result {
            Ok(path) => {
                let mut current = self.current.lock();
                if let Some(ref mut t) = *current {
                    t.status = DownloadStatus::Completed(path);
                }
                Ok(current.clone())
            }
            Err(e) => {
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

        while attempt <= task.retry_count {
            if attempt > 0 {
                println!("T-Vault FUSE: Retrying download for '{}' (attempt {}/{})", 
                         task.file_name, attempt + 1, MAX_RETRIES);
                tokio::time::sleep(RETRY_DELAY).await;
            }

            {
                let mut current = self.current.lock();
                if let Some(ref mut t) = *current {
                    t.status = DownloadStatus::Downloading { progress: 0 };
                }
            }

            match self.download_file(task).await {
                Ok(path) => return Ok(path),
                Err(e) => {
                    last_error = e.to_string();
                    println!("T-Vault FUSE: Download attempt {} failed: {}", attempt + 1, last_error);
                    
                    let mut current = self.current.lock();
                    if let Some(ref mut t) = *current {
                        if matches!(t.status, DownloadStatus::Cancelled) {
                            return Err(anyhow::anyhow!("Download cancelled"));
                        }
                        t.retry_count = attempt + 1;
                    }
                }
            }

            attempt += 1;

            if attempt >= MAX_RETRIES {
                break;
            }
        }

        Err(anyhow::anyhow!("Download failed after {} retries: {}", MAX_RETRIES, last_error))
    }

    async fn download_file(&self, task: &DownloadTask) -> Result<PathBuf> {
        let file_id = task.file_id.clone();
        let destination = task.destination.to_string_lossy().to_string();
        let client_ref = self.client_ref.clone();
        let file_name = task.file_name.clone();

        println!("T-Vault FUSE: Downloading '{}' to {}", file_name, destination);

        crate::storage::download_file(
            client_ref,
            &file_id,
            &destination,
            move |_, downloaded, total| {
                if total > 0 {
                    let progress = (downloaded as f64 / total as f64 * 100.0) as u8;
                    println!("T-Vault FUSE: Download progress for '{}': {}%", file_name, progress);
                }
            },
        ).await?;

        let path = PathBuf::from(&destination);
        if path.exists() {
            println!("T-Vault FUSE: Download complete for '{}'", task.file_name);
            Ok(path)
        } else {
            Err(anyhow::anyhow!("Download completed but file not found at {}", destination))
        }
    }

    pub fn check_disk_space(&self, required_bytes: u64, destination: &PathBuf) -> Result<()> {
        if let Some(parent) = destination.parent() {
            let available = fs_available_space(parent);
            if available < required_bytes {
                return Err(anyhow::anyhow!(
                    "Not enough disk space. Required: {} bytes, Available: {} bytes",
                    required_bytes, available
                ));
            }
        }
        Ok(())
    }

    pub fn cleanup_partial(&self, path: &PathBuf) {
        if path.exists() {
            if let Err(e) = std::fs::remove_file(path) {
                eprintln!("T-Vault FUSE: Failed to cleanup partial file: {}", e);
            } else {
                println!("T-Vault FUSE: Cleaned up partial file at {}", path.display());
            }
        }
    }
}

fn fs_available_space(path: &std::path::Path) -> u64 {
    #[cfg(target_os = "macos")]
    {
        use std::ffi::CString;
        use libc::{statfs, strlen};
        
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
