use fuser::{
    FileAttr, FileType, Filesystem, Request, ReplyData, ReplyEntry, 
    ReplyAttr, ReplyDirectory, ReplyOpen, ReplyEmpty, ReplyWrite,
    ReplyStatfs, ReplyXattr, KernelConfig, FUSE_ROOT_ID,
};
use std::ffi::OsStr;
use std::collections::HashMap;
use std::path::PathBuf;
use std::time::{Duration, Instant, UNIX_EPOCH};
use parking_lot::Mutex;
use grammers_client::Client;
use std::sync::Arc;
use tokio::sync::Mutex as AsyncMutex;
use libc::c_int;

use crate::storage::FileMetadata;
use super::inode_manager::InodeManager;
use super::cache::{MetadataCache, FileCache};
use super::download_queue::{DownloadQueue, DownloadStatus};
use super::open_tracker::{OpenTracker, FileInfo};

const TTL: Duration = Duration::from_secs(60);
const BLOCK_SIZE: u64 = 512;
const MAX_NAME_LENGTH: u32 = 255;
const ENOSYS: c_int = 78;
const ENOENT: c_int = 2;
const EISDIR: c_int = 21;
const EIO: c_int = 5;
const EBADF: c_int = 9;
const EACCES: c_int = 13;

struct OpenHandle {
    file_id: String,
    file_name: String,
    size: u64,
    destination: Option<PathBuf>,
    write_buffer: Option<Vec<u8>>,
    is_dirty: bool,
}

pub struct TVaultFS {
    client_ref: Arc<AsyncMutex<Option<Client>>>,
    inode_manager: InodeManager,
    metadata_cache: MetadataCache,
    file_cache: FileCache,
    download_queue: Arc<DownloadQueue>,
    open_tracker: Arc<OpenTracker>,
    handles: Mutex<HashMap<u64, OpenHandle>>,
    next_handle: Mutex<u64>,
}

macro_rules! fuse_log {
    ($($arg:tt)*) => {
        println!("🔍 [FUSE] {}", format!($($arg)*))
    };
}

macro_rules! fuse_warn {
    ($($arg:tt)*) => {
        eprintln!("⚠️  [FUSE] {}", format!($($arg)*))
    };
}

macro_rules! fuse_step {
    ($step:expr, $($arg:tt)*) => {
        println!("👉 [FUSE::{}] {}", $step, format!($($arg)*))
    };
}

impl TVaultFS {
    pub fn new(client_ref: Arc<AsyncMutex<Option<Client>>>) -> Self {
        fuse_log!("Creating new TVaultFS instance");
        let download_queue = Arc::new(DownloadQueue::new(client_ref.clone()));
        let open_tracker = Arc::new(OpenTracker::new());
        Self {
            client_ref,
            inode_manager: InodeManager::new(),
            metadata_cache: MetadataCache::new(),
            file_cache: FileCache::new(),
            download_queue,
            open_tracker,
            handles: Mutex::new(HashMap::new()),
            next_handle: Mutex::new(1),
        }
    }

    pub fn new_with_shared(
        client_ref: Arc<AsyncMutex<Option<Client>>>,
        open_tracker: Arc<OpenTracker>,
        download_queue: Arc<DownloadQueue>,
    ) -> Self {
        fuse_log!("Creating new TVaultFS with shared tracker/queue");
        Self {
            client_ref,
            inode_manager: InodeManager::new(),
            metadata_cache: MetadataCache::new(),
            file_cache: FileCache::new(),
            download_queue,
            open_tracker,
            handles: Mutex::new(HashMap::new()),
            next_handle: Mutex::new(1),
        }
    }

    pub fn set_app_handle(&self, handle: tauri::AppHandle) {
        fuse_log!("Setting Tauri app handle for dialog events");
        self.open_tracker.set_app_handle(handle);
    }

    pub fn respond_dialog(&self, result: super::open_tracker::DialogResult) {
        fuse_log!("Received dialog response from frontend");
        self.open_tracker.respond(result);
    }

    pub fn get_download_tasks(&self) -> Vec<super::download_queue::DownloadTask> {
        self.download_queue.get_all_tasks()
    }

    pub fn refresh_metadata(&self) -> anyhow::Result<()> {
        fuse_log!("Refreshing metadata from disk");
        self.metadata_cache.load_from_disk()
    }

    fn metadata_to_attr(&self, file: &FileMetadata, ino: u64) -> FileAttr {
        let created = UNIX_EPOCH + Duration::from_secs(file.created_at as u64);
        
        FileAttr {
            ino,
            size: file.size,
            blocks: (file.size + BLOCK_SIZE - 1) / BLOCK_SIZE,
            atime: created,
            mtime: created,
            ctime: created,
            crtime: created,
            kind: if file.is_folder { FileType::Directory } else { FileType::RegularFile },
            perm: if file.is_folder { 0o755 } else { 0o644 },
            nlink: if file.is_folder { 2 } else { 1 },
            uid: unsafe { libc::getuid() },
            gid: unsafe { libc::getgid() },
            rdev: 0,
            flags: 0,
            blksize: BLOCK_SIZE as u32,
        }
    }

    fn get_root_attr() -> FileAttr {
        FileAttr {
            ino: FUSE_ROOT_ID,
            size: 0,
            blocks: 0,
            atime: UNIX_EPOCH,
            mtime: UNIX_EPOCH,
            ctime: UNIX_EPOCH,
            crtime: UNIX_EPOCH,
            kind: FileType::Directory,
            perm: 0o755,
            nlink: 2,
            uid: unsafe { libc::getuid() },
            gid: unsafe { libc::getgid() },
            rdev: 0,
            flags: 0,
            blksize: BLOCK_SIZE as u32,
        }
    }

    fn next_handle(&self) -> u64 {
        let mut next = self.next_handle.lock();
        let handle = *next;
        *next += 1;
        handle
    }
}

impl Filesystem for TVaultFS {
    fn init(&mut self, _req: &Request, _config: &mut KernelConfig) -> Result<(), c_int> {
        fuse_step!("INIT", "Initializing filesystem...");
        match self.metadata_cache.load_from_disk() {
            Ok(_) => {
                fuse_log!("Filesystem initialized successfully - metadata loaded");
                Ok(())
            }
            Err(e) => {
                fuse_warn!("Failed to load metadata: {}", e);
                Err(EIO)
            }
        }
    }

    fn destroy(&mut self) {
        fuse_step!("DESTROY", "Destroying filesystem...");
        self.inode_manager.clear_cache();
        self.metadata_cache.clear_cache();
        self.download_queue.cancel_all();
        self.open_tracker.cancel_all();
        let mut handles = self.handles.lock();
        let count = handles.len();
        handles.clear();
        fuse_log!("Destroyed - cleared {} handles, cancelled all downloads", count);
    }

    fn lookup(&mut self, _req: &Request, parent: u64, name: &OsStr, reply: ReplyEntry) {
        let parent_path = if parent == FUSE_ROOT_ID {
            "/".to_string()
        } else {
            self.inode_manager.inode_to_path(parent)
        };

        let name_str = name.to_string_lossy();
        fuse_step!("LOOKUP", "parent='{}' name='{}'", parent_path, name_str);
        
        if let Some(file) = self.metadata_cache.find_file_in_folder(&parent_path, &name_str) {
            let ino = self.inode_manager.file_to_inode(&file);
            let attr = self.metadata_to_attr(&file, ino);
            fuse_log!("LOOKUP found: '{}' -> ino={}", name_str, ino);
            reply.entry(&TTL, &attr, 0);
            return;
        }

        fuse_log!("LOOKUP not found: '{}' in '{}'", name_str, parent_path);
        reply.error(ENOENT);
    }

    fn forget(&mut self, _req: &Request, ino: u64, _nlookup: u64) {
        fuse_step!("FORGET", "ino={}", ino);
        self.inode_manager.invalidate(ino);
    }

    fn getattr(&mut self, _req: &Request, ino: u64, reply: ReplyAttr) {
        if ino == FUSE_ROOT_ID {
            reply.attr(&TTL, &Self::get_root_attr());
            return;
        }

        if let Some(file) = self.inode_manager.inode_to_file(ino) {
            let attr = self.metadata_to_attr(&file, ino);
            reply.attr(&TTL, &attr);
            return;
        }

        fuse_warn!("GETATTR not found: ino={}", ino);
        reply.error(ENOENT);
    }

    fn opendir(&mut self, _req: &Request, _ino: u64, _flags: i32, reply: ReplyOpen) {
        fuse_step!("OPENDIR", "ino={}", _ino);
        reply.opened(0, 0);
    }

    fn releasedir(&mut self, _req: &Request, _ino: u64, _fh: u64, _flags: i32, reply: ReplyEmpty) {
        fuse_step!("RELEASEDIR", "ino={} fh={}", _ino, _fh);
        reply.ok();
    }

    fn readdir(&mut self, _req: &Request, ino: u64, _fh: u64, offset: i64, mut reply: ReplyDirectory) {
        let path = if ino == FUSE_ROOT_ID {
            "/".to_string()
        } else {
            self.inode_manager.inode_to_path(ino)
        };

        let files = self.metadata_cache.list_files(&path);
        fuse_step!("READDIR", "path='{}' offset={} found {} files", path, offset, files.len());
        
        let mut entries: Vec<(u64, FileType, &str)> = vec![
            (FUSE_ROOT_ID, FileType::Directory, "."),
            (FUSE_ROOT_ID, FileType::Directory, ".."),
        ];

        for file in &files {
            let child_ino = self.inode_manager.file_to_inode(file);
            let kind = if file.is_folder { FileType::Directory } else { FileType::RegularFile };
            entries.push((child_ino, kind, &file.name));
        }

        for (i, entry) in entries.iter().enumerate().skip(offset as usize) {
            let (child_ino, kind, name) = *entry;
            if reply.add(child_ino, (i + 1) as i64, kind, name) {
                break;
            }
        }
        reply.ok();
    }

    fn open(&mut self, _req: &Request, ino: u64, _flags: i32, reply: ReplyOpen) {
        let fh = self.next_handle();
        let open_start = Instant::now();
        fuse_step!("OPEN", "ino={} fh={} flags={}", ino, fh, _flags);

        if ino == FUSE_ROOT_ID {
            fuse_log!("OPEN: root directory, returning fh={}", fh);
            reply.opened(fh, 0);
            return;
        }

        if let Some(file) = self.inode_manager.inode_to_file(ino) {
            let file_id = file.id.clone();
            let file_name = file.name.clone();
            let file_size = file.size;
            fuse_log!("OPEN: file='{}' id='{}' size={}", file_name, file_id, file_size);

            // Step 1: Check if file is already in cache
            if self.file_cache.get(&file_id).is_some() {
                fuse_step!("OPEN→CACHE_HIT", "File '{}' already cached, opening directly", file_name);
                let handle = OpenHandle {
                    file_id: file_id.clone(),
                    file_name: file_name.clone(),
                    size: file_size,
                    destination: None,
                    write_buffer: None,
                    is_dirty: false,
                };
                self.handles.lock().insert(fh, handle);
                fuse_log!("OPEN complete (cached): fh={} in {}ms", fh, open_start.elapsed().as_millis());
                reply.opened(fh, 0);
                return;
            }
            fuse_step!("OPEN→CACHE_MISS", "File '{}' NOT in cache", file_name);

            // Step 2: Check if already downloading
            if self.download_queue.has_pending(&file_id) {
                fuse_step!("OPEN→ALREADY_DOWNLOADING", "File '{}' is already in download queue", file_name);
                let handle = OpenHandle {
                    file_id: file_id.clone(),
                    file_name: file_name.clone(),
                    size: file_size,
                    destination: None,
                    write_buffer: None,
                    is_dirty: false,
                };
                self.handles.lock().insert(fh, handle);
                reply.opened(fh, 0);
                return;
            }

            // Step 3: Request dialog - ask user where to save
            fuse_step!("OPEN→REQUEST_DIALOG", "Requesting save dialog from user for '{}'", file_name);
            let file_info = FileInfo {
                file_id: file_id.clone(),
                file_name: file_name.clone(),
                file_size,
            };

            let destination = match self.open_tracker.request_open(file_info) {
                Some(path) => {
                    fuse_step!("OPEN→DIALOG_RESPONSE", "User chose location: {}", path.display());
                    path
                }
                None => {
                    fuse_warn!("User cancelled or timed out for '{}'", file_name);
                    reply.error(EACCES);
                    return;
                }
            };

            // Step 4: Check disk space
            fuse_step!("OPEN→CHECK_DISK", "Checking disk space for {} bytes at {}", file_size, destination.display());
            if let Err(e) = self.download_queue.check_disk_space(file_size, &destination) {
                fuse_warn!("Not enough disk space: {}", e);
                reply.error(EIO);
                return;
            }
            fuse_log!("Disk space OK");

            // Step 5: Enqueue download
            fuse_step!("OPEN→ENQUEUE", "Enqueuing download: '{}' -> {}", file_name, destination.display());
            self.download_queue.enqueue(
                file_id.clone(),
                file_name.clone(),
                file_size,
                destination.clone(),
            );

            let handle = OpenHandle {
                file_id: file_id.clone(),
                file_name: file_name.clone(),
                size: file_size,
                destination: Some(destination.clone()),
                write_buffer: None,
                is_dirty: false,
            };
            self.handles.lock().insert(fh, handle);

            // Step 6: Spawn download thread
            fuse_step!("OPEN→SPAWN_DOWNLOAD", "Spawning download thread for '{}'", file_name);
            let queue = self.download_queue.clone();
            std::thread::spawn(move || {
                fuse_log!("Download thread started for file_id='{}'", file_id);
                let rt = tokio::runtime::Handle::current();
                rt.block_on(async {
                    if let Err(e) = queue.process_next().await {
                        fuse_warn!("Download thread error: {}", e);
                    }
                });
                fuse_log!("Download thread finished for file_id='{}'", file_id);
            });

            fuse_log!("OPEN complete (downloading): fh={} in {}ms", fh, open_start.elapsed().as_millis());
            reply.opened(fh, 0);
            return;
        }

        fuse_warn!("OPEN: ino={} not found in inode manager", ino);
        reply.error(ENOENT);
    }

    fn read(&mut self, _req: &Request, ino: u64, _fh: u64, offset: i64, size: u32, 
            _flags: i32, _lock_owner: Option<u64>, reply: ReplyData) {
        let read_start = Instant::now();
        fuse_step!("READ", "ino={} fh={} offset={} size={}", ino, _fh, offset, size);
        
        if ino == FUSE_ROOT_ID {
            reply.error(EISDIR);
            return;
        }

        let file_id = match self.inode_manager.get_file_id(ino) {
            Some(id) => {
                fuse_log!("READ: resolved ino={} -> file_id='{}'", ino, id);
                id
            }
            None => {
                fuse_warn!("READ: cannot resolve ino={}", ino);
                reply.error(ENOENT);
                return;
            }
        };

        // Attempt 1: Read from FUSE cache
        if let Some(cached_path) = self.file_cache.get(&file_id) {
            fuse_step!("READ→FUSE_CACHE", "File in FUSE cache at {}", cached_path.display());
            match std::fs::read(&cached_path) {
                Ok(data) => {
                    let end = std::cmp::min(offset as usize + size as usize, data.len());
                    if offset as usize <= data.len() {
                        fuse_log!("READ complete (FUSE cache): {} bytes at offset {} in {}ms", 
                                  end - offset as usize, offset, read_start.elapsed().as_millis());
                        reply.data(&data[offset as usize..end]);
                    } else {
                        reply.data(&[]);
                    }
                    return;
                }
                Err(e) => {
                    fuse_warn!("READ: FUSE cache read failed: {}", e);
                }
            }
        }

        // Attempt 2: Read from download destination
        {
            let handles = self.handles.lock();
            if let Some(handle) = handles.get(&_fh) {
                if let Some(ref dest) = handle.destination {
                    if dest.exists() {
                        fuse_step!("READ→DEST_FILE", "File at download destination: {}", dest.display());
                        match std::fs::read(dest) {
                            Ok(data) => {
                                fuse_log!("READ: Read {} bytes from destination, caching...", data.len());
                                let _ = self.file_cache.put(&file_id, &data);
                                let end = std::cmp::min(offset as usize + size as usize, data.len());
                                if offset as usize <= data.len() {
                                    fuse_log!("READ complete (destination): {} bytes in {}ms", 
                                              end - offset as usize, read_start.elapsed().as_millis());
                                    reply.data(&data[offset as usize..end]);
                                } else {
                                    reply.data(&[]);
                                }
                                return;
                            }
                            Err(e) => {
                                fuse_warn!("READ: Destination file read failed: {}", e);
                            }
                        }
                    } else {
                        fuse_step!("READ→DEST_NOT_YET", "Download destination doesn't exist yet: {}", dest.display());
                    }
                }
            }
        }

        // Attempt 3: Check download queue status
        let status = self.download_queue.get_status(&file_id);
        fuse_step!("READ→QUEUE_STATUS", "File '{}' status: {:?}", file_id, match &status {
            DownloadStatus::Pending => "Pending".to_string(),
            DownloadStatus::Downloading { progress } => format!("Downloading({}%)", progress),
            DownloadStatus::Completed(_) => "Completed".to_string(),
            DownloadStatus::Failed(e) => format!("Failed({})", e),
            DownloadStatus::Cancelled => "Cancelled".to_string(),
        });

        match status {
            DownloadStatus::Downloading { progress } => {
                fuse_warn!("READ: File still downloading ({}%), returning EIO - Finder will retry", progress);
                reply.error(EIO);
            }
            DownloadStatus::Pending => {
                fuse_warn!("READ: File pending download, returning EIO - Finder will retry");
                reply.error(EIO);
            }
            DownloadStatus::Completed(path) => {
                fuse_step!("READ→COMPLETED", "Download completed at {}", path.display());
                match std::fs::read(&path) {
                    Ok(data) => {
                        fuse_log!("READ: Caching {} bytes from completed download", data.len());
                        let _ = self.file_cache.put(&file_id, &data);
                        let end = std::cmp::min(offset as usize + size as usize, data.len());
                        if offset as usize <= data.len() {
                            fuse_log!("READ complete (downloaded): {} bytes in {}ms", 
                                      end - offset as usize, read_start.elapsed().as_millis());
                            reply.data(&data[offset as usize..end]);
                        } else {
                            reply.data(&[]);
                        }
                    }
                    Err(e) => {
                        fuse_warn!("READ: Failed to read completed download: {}", e);
                        reply.error(EIO);
                    }
                }
            }
            DownloadStatus::Failed(e) => {
                fuse_warn!("READ: Download failed for '{}': {}", file_id, e);
                let handles = self.handles.lock();
                if let Some(handle) = handles.get(&_fh) {
                    if let Some(ref dest) = handle.destination {
                        fuse_step!("READ→CLEANUP", "Cleaning up partial file at {}", dest.display());
                        self.download_queue.cleanup_partial(dest);
                    }
                }
                reply.error(EIO);
            }
            DownloadStatus::Cancelled => {
                fuse_log!("READ: Download was cancelled for '{}'", file_id);
                reply.error(EACCES);
            }
            _ => {
                fuse_warn!("READ: Unknown status for '{}'", file_id);
                reply.error(EIO);
            }
        }
    }

    fn write(&mut self, _req: &Request, _ino: u64, fh: u64, offset: i64, data: &[u8], 
            _write_flags: u32, _flags: i32, _lock_owner: Option<u64>, reply: ReplyWrite) {
        fuse_step!("WRITE", "fh={} offset={} size={}", fh, offset, data.len());
        
        let mut handles = self.handles.lock();
        
        if let Some(handle) = handles.get_mut(&fh) {
            handle.is_dirty = true;
            
            let buffer = handle.write_buffer.get_or_insert_with(|| {
                let mut buf = Vec::new();
                if offset > 0 {
                    buf.resize(offset as usize, 0);
                }
                buf
            });
            
            let end = offset as usize + data.len();
            if buffer.len() < end {
                buffer.resize(end, 0);
            }
            
            buffer[offset as usize..end].copy_from_slice(data);
            handle.size = buffer.len() as u64;
            
            reply.written(data.len() as u32);
            return;
        }

        fuse_warn!("WRITE: handle {} not found", fh);
        reply.error(EBADF);
    }

    fn release(&mut self, _req: &Request, _ino: u64, fh: u64, _flags: i32, 
               _lock_owner: Option<u64>, _flush: bool, reply: ReplyEmpty) {
        fuse_step!("RELEASE", "fh={}", fh);
        
        let handle = self.handles.lock().remove(&fh);
        
        if let Some(handle) = handle {
            fuse_log!("RELEASE: file='{}' dirty={}", handle.file_name, handle.is_dirty);
            if handle.is_dirty {
                if let Some(buffer) = handle.write_buffer {
                    fuse_log!("RELEASE: File '{}' modified ({} bytes), would upload to Telegram", 
                             handle.file_name, buffer.len());
                    self.file_cache.put(&handle.file_id, &buffer).ok();
                }
            }
        } else {
            fuse_warn!("RELEASE: handle {} not found", fh);
        }
        
        reply.ok();
    }

    fn statfs(&mut self, _req: &Request, _ino: u64, reply: ReplyStatfs) {
        let stats = self.metadata_cache.get_stats();
        
        let total_blocks = 10000000u64;
        let free_blocks = total_blocks.saturating_sub(stats.total_size / BLOCK_SIZE);
        
        reply.statfs(
            total_blocks,
            free_blocks,
            free_blocks,
            stats.total_files + 100000,
            100000,
            BLOCK_SIZE as u32,
            MAX_NAME_LENGTH,
            BLOCK_SIZE as u32,
        );
    }

    fn flush(&mut self, _req: &Request, _ino: u64, _fh: u64, _lock_owner: u64, reply: ReplyEmpty) {
        reply.ok();
    }

    fn fsync(&mut self, _req: &Request, _ino: u64, _fh: u64, _datasync: bool, reply: ReplyEmpty) {
        reply.ok();
    }

    fn getxattr(&mut self, _req: &Request, _ino: u64, _name: &OsStr, _size: u32, reply: ReplyXattr) {
        reply.error(ENOSYS);
    }

    fn setxattr(&mut self, _req: &Request, _ino: u64, _name: &OsStr, _value: &[u8], 
                _flags: i32, _position: u32, reply: ReplyEmpty) {
        reply.error(ENOSYS);
    }

    fn listxattr(&mut self, _req: &Request, _ino: u64, _size: u32, reply: ReplyXattr) {
        reply.error(ENOSYS);
    }

    fn removexattr(&mut self, _req: &Request, _ino: u64, _name: &OsStr, reply: ReplyEmpty) {
        reply.error(ENOSYS);
    }
}
