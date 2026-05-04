use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use parking_lot::Mutex;
use lru::LruCache;
use std::num::NonZeroUsize;
use std::time::SystemTime;
use anyhow::Result;

use crate::storage::{FileMetadata, MetadataStore, StorageStats};

pub struct MetadataCache {
    store: Mutex<Option<MetadataStore>>,
    folder_cache: Mutex<LruCache<String, Vec<FileMetadata>>>,
    last_load: Mutex<Option<SystemTime>>,
}

impl MetadataCache {
    pub fn new() -> Self {
        Self {
            store: Mutex::new(None),
            folder_cache: Mutex::new(LruCache::new(NonZeroUsize::new(100).unwrap())),
            last_load: Mutex::new(None),
        }
    }

    pub fn load_from_disk(&self) -> Result<()> {
        let data_dir = directories::ProjectDirs::from("com", "tvault", "t-vault")
            .ok_or_else(|| anyhow::anyhow!("Failed to get data directory"))?
            .data_dir()
            .to_path_buf();
        
        let metadata_path = data_dir.join("metadata.json");
        
        if !metadata_path.exists() {
            let mut store = self.store.lock();
            *store = Some(MetadataStore::new());
            return Ok(());
        }

        let content = std::fs::read_to_string(&metadata_path)?;
        let store: MetadataStore = serde_json::from_str(&content)?;
        
        {
            let mut s = self.store.lock();
            *s = Some(store);
        }
        
        {
            let mut last = self.last_load.lock();
            *last = Some(SystemTime::now());
        }
        
        self.folder_cache.lock().clear();
        
        Ok(())
    }

    pub fn list_files(&self, folder: &str) -> Vec<FileMetadata> {
        if let Some(cached) = self.folder_cache.lock().get(folder) {
            return cached.clone();
        }
        
        let store = self.store.lock();
        if let Some(ref s) = *store {
            let files: Vec<FileMetadata> = s.files.iter()
                .filter(|f| f.folder == folder)
                .cloned()
                .collect();
            
            self.folder_cache.lock().put(folder.to_string(), files.clone());
            return files;
        }
        
        Vec::new()
    }

    pub fn get_file_by_id(&self, id: &str) -> Option<FileMetadata> {
        let store = self.store.lock();
        if let Some(ref s) = *store {
            return s.files.iter().find(|f| f.id == id).cloned();
        }
        None
    }

    pub fn get_stats(&self) -> StorageStats {
        let store = self.store.lock();
        if let Some(ref s) = *store {
            let total_size: u64 = s.files.iter().filter(|f| !f.is_folder).map(|f| f.size).sum();
            let total_files = s.files.iter().filter(|f| !f.is_folder).count() as u64;
            let folder_count = s.folders.len() as u64;
            
            return StorageStats {
                total_files,
                total_size,
                folder_count,
            };
        }
        
        StorageStats {
            total_files: 0,
            total_size: 0,
            folder_count: 0,
        }
    }

    pub fn get_folders(&self) -> Vec<String> {
        let store = self.store.lock();
        if let Some(ref s) = *store {
            return s.folders.clone();
        }
        Vec::new()
    }

    pub fn find_file_in_folder(&self, folder: &str, name: &str) -> Option<FileMetadata> {
        let files = self.list_files(folder);
        files.into_iter().find(|f| f.name == name)
    }

    pub fn invalidate_folder(&self, folder: &str) {
        self.folder_cache.lock().pop(folder);
    }

    pub fn clear_cache(&self) {
        self.folder_cache.lock().clear();
    }
}

impl Default for MetadataCache {
    fn default() -> Self {
        Self::new()
    }
}

const MAX_CACHE_SIZE: u64 = 500 * 1024 * 1024;

pub struct FileCache {
    cache_dir: PathBuf,
    current_size: Mutex<u64>,
    access_times: Mutex<HashMap<String, SystemTime>>,
}

impl FileCache {
    pub fn new() -> Self {
        let cache_dir = directories::ProjectDirs::from("com", "tvault", "t-vault")
            .map(|d| d.cache_dir().join("fuse-cache").to_path_buf())
            .unwrap_or_else(|| std::env::temp_dir().join("tvault-fuse-cache"));
        
        let _ = std::fs::create_dir_all(&cache_dir);
        
        Self {
            cache_dir,
            current_size: Mutex::new(0),
            access_times: Mutex::new(HashMap::new()),
        }
    }

    pub fn get(&self, file_id: &str) -> Option<PathBuf> {
        let safe_id = sanitize_filename(file_id);
        let cached = self.cache_dir.join(&safe_id);
        
        if cached.exists() {
            self.access_times.lock().insert(file_id.to_string(), SystemTime::now());
            return Some(cached);
        }
        None
    }

    pub fn put(&self, file_id: &str, data: &[u8]) -> std::io::Result<PathBuf> {
        self.evict_if_needed(data.len() as u64)?;
        
        let safe_id = sanitize_filename(file_id);
        let path = self.cache_dir.join(&safe_id);
        
        std::fs::write(&path, data)?;
        
        {
            let mut size = self.current_size.lock();
            *size += data.len() as u64;
        }
        
        self.access_times.lock().insert(file_id.to_string(), SystemTime::now());
        
        Ok(path)
    }

    pub fn remove(&self, file_id: &str) {
        let safe_id = sanitize_filename(file_id);
        let path = self.cache_dir.join(&safe_id);
        
        if path.exists() {
            if let Ok(metadata) = std::fs::metadata(&path) {
                let size = metadata.len();
                let _ = std::fs::remove_file(&path);
                
                let mut current = self.current_size.lock();
                *current = current.saturating_sub(size);
            }
        }
        
        self.access_times.lock().remove(file_id);
    }

    fn evict_if_needed(&self, needed_size: u64) -> std::io::Result<()> {
        let mut current_size = self.current_size.lock();
        
        if *current_size + needed_size > MAX_CACHE_SIZE {
            let to_remove: Vec<String> = {
                let access_times = self.access_times.lock();
                let mut entries: Vec<_> = access_times.iter().collect();
                entries.sort_by_key(|(_, time)| *time);
                entries.into_iter()
                    .take_while(|_| *current_size + needed_size > MAX_CACHE_SIZE)
                    .map(|(id, _)| id.clone())
                    .collect()
            };
            
            for file_id in to_remove {
                let safe_id = sanitize_filename(&file_id);
                let path = self.cache_dir.join(&safe_id);
                
                if path.exists() {
                    if let Ok(metadata) = std::fs::metadata(&path) {
                        let size = metadata.len();
                        let _ = std::fs::remove_file(&path);
                        *current_size = current_size.saturating_sub(size);
                    }
                }
                
                self.access_times.lock().remove(&file_id);
            }
        }
        
        Ok(())
    }

    pub fn clear(&self) -> std::io::Result<()> {
        for entry in std::fs::read_dir(&self.cache_dir)? {
            if let Ok(entry) = entry {
                let _ = std::fs::remove_file(entry.path());
            }
        }
        
        *self.current_size.lock() = 0;
        self.access_times.lock().clear();
        
        Ok(())
    }
}

impl Default for FileCache {
    fn default() -> Self {
        Self::new()
    }
}

fn sanitize_filename(name: &str) -> String {
    name.chars()
        .map(|c| match c {
            '/' | '\\' | ':' | '*' | '?' | '"' | '<' | '>' | '|' => '_',
            _ => c,
        })
        .collect()
}
