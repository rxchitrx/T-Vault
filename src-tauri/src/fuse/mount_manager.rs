use std::path::{Path, PathBuf};
use std::sync::Arc;
use parking_lot::Mutex;
use grammers_client::Client;
use tokio::sync::Mutex as AsyncMutex;
use anyhow::Result;

use super::filesystem::TVaultFS;

pub struct MountHandle {
    pub mountpoint: PathBuf,
    thread_handle: Option<std::thread::JoinHandle<()>>,
    terminate_flag: Arc<Mutex<bool>>,
}

impl MountHandle {
    pub fn is_mounted(&self) -> bool {
        self.thread_handle.is_some() && !*self.terminate_flag.lock()
    }
}

impl Drop for MountHandle {
    fn drop(&mut self) {
        *self.terminate_flag.lock() = true;
        
        if let Some(handle) = self.thread_handle.take() {
            let mountpoint = self.mountpoint.clone();
            
            std::thread::spawn(move || {
                #[cfg(target_os = "macos")]
                {
                    let _ = std::process::Command::new("umount")
                        .arg(&mountpoint)
                        .output();
                }
                
                #[cfg(target_os = "linux")]
                {
                    let _ = std::process::Command::new("fusermount")
                        .args(["-u", &mountpoint.to_string_lossy()])
                        .output();
                }
            });
            
            let _ = handle.join();
        }
    }
}

pub struct MountManager {
    handle: Mutex<Option<MountHandle>>,
    client_ref: Arc<AsyncMutex<Option<Client>>>,
}

impl MountManager {
    pub fn new(client_ref: Arc<AsyncMutex<Option<Client>>>) -> Self {
        Self {
            handle: Mutex::new(None),
            client_ref,
        }
    }

    pub fn mount<P: AsRef<Path>>(&self, mountpoint: P) -> Result<String> {
        let mountpoint = mountpoint.as_ref().to_path_buf();
        
        {
            let handle = self.handle.lock();
            if handle.is_some() && handle.as_ref().unwrap().is_mounted() {
                return Err(anyhow::anyhow!("Already mounted at {}", 
                    handle.as_ref().unwrap().mountpoint.display()));
            }
        }

        std::fs::create_dir_all(&mountpoint)?;

        let client_ref = self.client_ref.clone();
        let mountpoint_clone = mountpoint.clone();
        let terminate_flag = Arc::new(Mutex::new(false));

        let filesystem = TVaultFS::new(client_ref);

        let (ready_tx, ready_rx) = std::sync::mpsc::channel::<Result<()>>();

        let thread_handle = std::thread::spawn(move || {
            let options = vec![
                fuser::MountOption::FSName("t-vault".to_string()),
                fuser::MountOption::AutoUnmount,
            ];

            println!("T-Vault FUSE: Attempting to mount at {}", mountpoint_clone.display());
            
            match fuser::mount2(filesystem, &mountpoint_clone, &options) {
                Ok(_) => {
                    println!("T-Vault FUSE: Mount ended normally");
                    let _ = ready_tx.send(Ok(()));
                }
                Err(e) => {
                    eprintln!("T-Vault FUSE: Mount error: {}", e);
                    let _ = ready_tx.send(Err(anyhow::anyhow!("Mount failed: {}", e)));
                }
            }
        });

        *self.handle.lock() = Some(MountHandle {
            mountpoint: mountpoint.clone(),
            thread_handle: Some(thread_handle),
            terminate_flag,
        });

        println!("T-Vault FUSE: Mounted at {}", mountpoint.display());
        Ok(format!("Mounted at {}", mountpoint.display()))
    }

    pub fn unmount(&self) -> Result<()> {
        let mut handle_guard = self.handle.lock();
        
        if let Some(mut handle) = handle_guard.take() {
            *handle.terminate_flag.lock() = true;

            #[cfg(target_os = "macos")]
            {
                let output = std::process::Command::new("umount")
                    .arg(&handle.mountpoint)
                    .output();
                
                match output {
                    Ok(o) if o.status.success() => {
                        println!("T-Vault FUSE: Unmounted successfully");
                    }
                    Ok(o) => {
                        eprintln!("T-Vault FUSE: umount failed: {}", String::from_utf8_lossy(&o.stderr));
                    }
                    Err(e) => {
                        eprintln!("T-Vault FUSE: Failed to run umount: {}", e);
                    }
                }
            }

            #[cfg(target_os = "linux")]
            {
                let _ = std::process::Command::new("fusermount")
                    .args(["-u", &handle.mountpoint.to_string_lossy()])
                    .output();
            }

            if let Some(thread_handle) = handle.thread_handle.take() {
                let _ = thread_handle.join();
            }

            Ok(())
        } else {
            Err(anyhow::anyhow!("Not mounted"))
        }
    }

    pub fn is_mounted(&self) -> bool {
        if let Some(handle) = self.handle.lock().as_ref() {
            return handle.is_mounted();
        }
        false
    }

    pub fn get_mountpoint(&self) -> Option<PathBuf> {
        self.handle.lock().as_ref().map(|h| h.mountpoint.clone())
    }

    pub fn default_mountpoint() -> PathBuf {
        #[cfg(target_os = "macos")]
        {
            std::env::var("HOME")
                .map(|h| PathBuf::from(h).join("T-Vault"))
                .unwrap_or_else(|_| PathBuf::from("/tmp/T-Vault"))
        }
        
        #[cfg(target_os = "linux")]
        {
            std::env::var("HOME")
                .map(|h| PathBuf::from(h).join(".mnt/t-vault"))
                .unwrap_or_else(|_| PathBuf::from("/mnt/t-vault"))
        }
        
        #[cfg(not(any(target_os = "macos", target_os = "linux")))]
        {
            std::env::temp_dir().join("t-vault")
        }
    }

    pub fn refresh_metadata(&self) -> Result<()> {
        Ok(())
    }
}

impl Drop for MountManager {
    fn drop(&mut self) {
        let _ = self.unmount();
    }
}
