use fuser::{
    FileAttr, FileType, Filesystem, Request, ReplyData, ReplyEntry, 
    ReplyAttr, ReplyDirectory, ReplyOpen, ReplyEmpty, ReplyWrite,
    ReplyStatfs, ReplyXattr, KernelConfig, FUSE_ROOT_ID,
};
use std::ffi::OsStr;
use std::collections::HashMap;
use std::time::{Duration, UNIX_EPOCH};
use parking_lot::Mutex;
use grammers_client::Client;
use std::sync::Arc;
use tokio::sync::Mutex as AsyncMutex;
use libc::c_int;

use crate::storage::FileMetadata;
use super::inode_manager::InodeManager;
use super::cache::{MetadataCache, FileCache};

const TTL: Duration = Duration::from_secs(60);
const BLOCK_SIZE: u64 = 512;
const MAX_NAME_LENGTH: u32 = 255;
const ENOSYS: c_int = 78;
const ENOENT: c_int = 2;
const EISDIR: c_int = 21;
const EIO: c_int = 5;
const EBADF: c_int = 9;

struct OpenHandle {
    file_id: String,
    file_name: String,
    size: u64,
    write_buffer: Option<Vec<u8>>,
    is_dirty: bool,
}

pub struct TVaultFS {
    client_ref: Arc<AsyncMutex<Option<Client>>>,
    inode_manager: InodeManager,
    metadata_cache: MetadataCache,
    file_cache: FileCache,
    handles: Mutex<HashMap<u64, OpenHandle>>,
    next_handle: Mutex<u64>,
}

impl TVaultFS {
    pub fn new(client_ref: Arc<AsyncMutex<Option<Client>>>) -> Self {
        Self {
            client_ref,
            inode_manager: InodeManager::new(),
            metadata_cache: MetadataCache::new(),
            file_cache: FileCache::new(),
            handles: Mutex::new(HashMap::new()),
            next_handle: Mutex::new(1),
        }
    }

    pub fn refresh_metadata(&self) -> anyhow::Result<()> {
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
        match self.metadata_cache.load_from_disk() {
            Ok(_) => {
                println!("T-Vault FUSE: Filesystem initialized");
                Ok(())
            }
            Err(e) => {
                eprintln!("T-Vault FUSE: Failed to load metadata: {}", e);
                Err(EIO)
            }
        }
    }

    fn destroy(&mut self) {
        println!("T-Vault FUSE: Filesystem destroyed");
        self.inode_manager.clear_cache();
        self.metadata_cache.clear_cache();
    }

    fn lookup(&mut self, _req: &Request, parent: u64, name: &OsStr, reply: ReplyEntry) {
        let parent_path = if parent == FUSE_ROOT_ID {
            "/".to_string()
        } else {
            self.inode_manager.inode_to_path(parent)
        };

        let name_str = name.to_string_lossy();
        
        if let Some(file) = self.metadata_cache.find_file_in_folder(&parent_path, &name_str) {
            let ino = self.inode_manager.file_to_inode(&file);
            let attr = self.metadata_to_attr(&file, ino);
            reply.entry(&TTL, &attr, 0);
            return;
        }

        reply.error(ENOENT);
    }

    fn forget(&mut self, _req: &Request, ino: u64, _nlookup: u64) {
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

        reply.error(ENOENT);
    }

    fn opendir(&mut self, _req: &Request, _ino: u64, _flags: i32, reply: ReplyOpen) {
        reply.opened(0, 0);
    }

    fn releasedir(&mut self, _req: &Request, _ino: u64, _fh: u64, _flags: i32, reply: ReplyEmpty) {
        reply.ok();
    }

    fn readdir(&mut self, _req: &Request, ino: u64, _fh: u64, offset: i64, mut reply: ReplyDirectory) {
        let path = if ino == FUSE_ROOT_ID {
            "/".to_string()
        } else {
            self.inode_manager.inode_to_path(ino)
        };

        let files = self.metadata_cache.list_files(&path);
        
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

        if ino == FUSE_ROOT_ID {
            reply.opened(fh, 0);
            return;
        }

        if let Some(file) = self.inode_manager.inode_to_file(ino) {
            let handle = OpenHandle {
                file_id: file.id.clone(),
                file_name: file.name.clone(),
                size: file.size,
                write_buffer: None,
                is_dirty: false,
            };
            
            self.handles.lock().insert(fh, handle);
            reply.opened(fh, 0);
            return;
        }

        reply.error(ENOENT);
    }

    fn read(&mut self, _req: &Request, ino: u64, _fh: u64, offset: i64, size: u32, 
            _flags: i32, _lock_owner: Option<u64>, reply: ReplyData) {
        
        if ino == FUSE_ROOT_ID {
            reply.error(EISDIR);
            return;
        }

        let file_id = match self.inode_manager.get_file_id(ino) {
            Some(id) => id,
            None => {
                reply.error(ENOENT);
                return;
            }
        };

        if let Some(cached_path) = self.file_cache.get(&file_id) {
            match std::fs::read(&cached_path) {
                Ok(data) => {
                    let end = std::cmp::min(offset as usize + size as usize, data.len());
                    if offset as usize <= data.len() {
                        reply.data(&data[offset as usize..end]);
                    } else {
                        reply.data(&[]);
                    }
                    return;
                }
                Err(e) => {
                    eprintln!("T-Vault FUSE: Failed to read cached file: {}", e);
                }
            }
        }

        reply.error(EIO);
    }

    fn write(&mut self, _req: &Request, _ino: u64, fh: u64, offset: i64, data: &[u8], 
            _write_flags: u32, _flags: i32, _lock_owner: Option<u64>, reply: ReplyWrite) {
        
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

        reply.error(EBADF);
    }

    fn release(&mut self, _req: &Request, _ino: u64, fh: u64, _flags: i32, 
               _lock_owner: Option<u64>, _flush: bool, reply: ReplyEmpty) {
        
        let handle = self.handles.lock().remove(&fh);
        
        if let Some(handle) = handle {
            if handle.is_dirty {
                if let Some(buffer) = handle.write_buffer {
                    println!("T-Vault FUSE: File '{}' modified ({} bytes), would upload to Telegram", 
                             handle.file_name, buffer.len());
                    
                    self.file_cache.put(&handle.file_id, &buffer).ok();
                }
            }
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
