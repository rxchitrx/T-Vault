use std::collections::HashMap;
use std::sync::Arc;
use parking_lot::Mutex;
use lru::LruCache;
use std::num::NonZeroUsize;

use crate::storage::FileMetadata;

pub const INODE_ROOT: u64 = 1;
const INODE_FOLDER_PREFIX: u64 = 0x01_00_00_00_00_00_00_00;
const INODE_FILE_PREFIX: u64 = 0x02_00_00_00_00_00_00_00;

pub struct InodeManager {
    inode_to_file: Mutex<LruCache<u64, FileMetadata>>,
    path_to_inode: Mutex<HashMap<String, u64>>,
    next_temp_inode: Mutex<u64>,
}

impl InodeManager {
    pub fn new() -> Self {
        Self {
            inode_to_file: Mutex::new(LruCache::new(NonZeroUsize::new(10000).unwrap())),
            path_to_inode: Mutex::new(HashMap::new()),
            next_temp_inode: Mutex::new(0x10_00_00_00_00_00_00_00),
        }
    }

    pub fn file_to_inode(&self, file: &FileMetadata) -> u64 {
        if file.is_folder {
            let path = &file.folder;
            let name = &file.name;
            let full_path = if path == "/" {
                format!("/{}", name)
            } else {
                format!("{}/{}", path, name)
            };
            
            if let Some(&ino) = self.path_to_inode.lock().get(&full_path) {
                return ino;
            }
            
            let ino = if let Some(chat_id) = file.chat_id {
                INODE_FOLDER_PREFIX | ((chat_id as u64) << 24)
            } else {
                let mut next = self.next_temp_inode.lock();
                let ino = *next;
                *next += 1;
                ino
            };
            
            self.path_to_inode.lock().insert(full_path.clone(), ino);
            self.inode_to_file.lock().put(ino, file.clone());
            ino
        } else {
            let chat_part = file.chat_id.unwrap_or(0) as u64;
            let msg_part = file.message_id.unwrap_or(0) as u64;
            
            let ino = INODE_FILE_PREFIX | (chat_part << 24) | msg_part;
            self.inode_to_file.lock().put(ino, file.clone());
            ino
        }
    }

    pub fn inode_to_file(&self, ino: u64) -> Option<FileMetadata> {
        if ino == INODE_ROOT {
            return None;
        }
        
        self.inode_to_file.lock().get(&ino).cloned()
    }

    pub fn inode_to_path(&self, ino: u64) -> String {
        if ino == INODE_ROOT {
            return "/".to_string();
        }
        
        if let Some(file) = self.inode_to_file.lock().get(&ino) {
            if file.is_folder {
                let path = &file.folder;
                let name = &file.name;
                return if path == "/" {
                    format!("/{}", name)
                } else {
                    format!("{}/{}", path, name)
                };
            } else {
                return file.folder.clone();
            }
        }
        
        "/".to_string()
    }

    pub fn get_file_id(&self, ino: u64) -> Option<String> {
        if ino == INODE_ROOT {
            return None;
        }
        
        self.inode_to_file.lock().get(&ino).map(|f| f.id.clone())
    }

    pub fn invalidate(&self, ino: u64) {
        self.inode_to_file.lock().pop(&ino);
    }

    pub fn invalidate_path(&self, path: &str) {
        if let Some(ino) = self.path_to_inode.lock().remove(path) {
            self.inode_to_file.lock().pop(&ino);
        }
    }

    pub fn clear_cache(&self) {
        self.inode_to_file.lock().clear();
        self.path_to_inode.lock().clear();
    }
}

impl Default for InodeManager {
    fn default() -> Self {
        Self::new()
    }
}
