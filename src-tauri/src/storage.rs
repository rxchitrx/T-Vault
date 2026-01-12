use serde::{Deserialize, Serialize};
use std::path::Path;
use anyhow::Result;
use grammers_client::Client;
use std::sync::Arc;
use tokio::sync::{Mutex, RwLock};
use tokio::io::{AsyncRead, ReadBuf};
use std::pin::Pin;
use std::task::{Context, Poll};
use lazy_static::lazy_static;

lazy_static! {
    static ref METADATA_CACHE: RwLock<Option<MetadataStore>> = RwLock::new(None);
}

pub struct ProgressReader<R> {
    inner: R,
    total_size: u64,
    current_size: u64,
    last_reported_progress: u32,
    last_reported_time: std::time::Instant,
    on_progress: Box<dyn Fn(u32) + Send + Sync>,
}

impl<R: AsyncRead + Unpin> ProgressReader<R> {
    pub fn new(inner: R, total_size: u64, on_progress: impl Fn(u32) + Send + Sync + 'static) -> Self {
        Self {
            inner,
            total_size,
            current_size: 0,
            last_reported_progress: 0,
            last_reported_time: std::time::Instant::now(),
            on_progress: Box::new(on_progress),
        }
    }
}

impl<R: AsyncRead + Unpin> AsyncRead for ProgressReader<R> {
    fn poll_read(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &mut ReadBuf<'_>,
    ) -> Poll<std::io::Result<()>> {
        let prev_len = buf.filled().len();
        match Pin::new(&mut self.inner).poll_read(cx, buf) {
            Poll::Ready(Ok(())) => {
                let read_len = buf.filled().len() - prev_len;
                if read_len > 0 {
                    self.current_size += read_len as u64;
                    
                    if self.total_size > 0 {
                        let progress = ((self.current_size as f64 / self.total_size as f64) * 100.0) as u32;
                        let now = std::time::Instant::now();
                        
                        // Throttle: only report if progress changed AND at least 100ms passed
                        // This prevents flooding the frontend with events for large files
                        if progress > self.last_reported_progress && 
                           (now.duration_since(self.last_reported_time).as_millis() > 100 || progress == 100) {
                            self.last_reported_progress = progress;
                            self.last_reported_time = now;
                            (self.on_progress)(progress);
                        }
                    }
                }
                Poll::Ready(Ok(()))
            }
            res => res,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileMetadata {
    pub id: String,
    pub name: String,
    pub size: u64,
    pub mime_type: String,
    pub created_at: i64,
    pub folder: String,
    pub is_folder: bool,
    pub thumbnail: Option<String>,
    pub message_id: Option<i32>,
    pub encrypted: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StorageStats {
    pub total_files: u64,
    pub total_size: u64,
    pub folder_count: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MetadataStore {
    pub files: Vec<FileMetadata>,
    pub folders: Vec<String>,
}

impl Default for MetadataStore {
    fn default() -> Self {
        Self::new()
    }
}

impl MetadataStore {
    pub fn new() -> Self {
        Self {
            files: Vec::new(),
            folders: vec!["/".to_string()],
        }
    }
}

// Reserved for future encryption feature
#[allow(dead_code)]
const ENCRYPTION_PASSWORD: &str = "tvault_secure_key_2024";
#[allow(dead_code)]
const METADATA_TAG: &str = "#TVAULT_METADATA_V1";

async fn get_metadata_path() -> Result<std::path::PathBuf> {
    // Use app data directory instead of current directory to avoid triggering Tauri rebuilds
    let data_dir = directories::ProjectDirs::from("com", "tvault", "t-vault")
        .ok_or_else(|| anyhow::anyhow!("Failed to get data directory"))?
        .data_dir()
        .to_path_buf();
    
    // Create directory if it doesn't exist
    tokio::fs::create_dir_all(&data_dir).await?;
    
    Ok(data_dir.join("metadata.json"))
}

async fn ensure_metadata_loaded() -> Result<()> {
    // Check if already loaded
    if METADATA_CACHE.read().await.is_some() {
        return Ok(());
    }

    // Cache miss - load from disk
    let path = get_metadata_path().await?;
    let metadata = if path.exists() {
        let data = tokio::fs::read_to_string(&path).await?;
        serde_json::from_str(&data)?
    } else {
        MetadataStore::new()
    };

    // Update cache
    let mut cache = METADATA_CACHE.write().await;
    *cache = Some(metadata);

    Ok(())
}

async fn load_metadata_copy() -> Result<MetadataStore> {
    ensure_metadata_loaded().await?;
    let cache = METADATA_CACHE.read().await;
    Ok(cache.as_ref().unwrap().clone())
}

async fn save_metadata_local(store: &MetadataStore) -> Result<()> {
    // Update cache first
    {
        let mut cache = METADATA_CACHE.write().await;
        *cache = Some(store.clone());
    }

    let path = get_metadata_path().await?;
    let data = serde_json::to_string_pretty(store)
        .map_err(|e| anyhow::anyhow!("Failed to serialize metadata: {}", e))?;
    
    // Write atomically: write to temp file first, then rename
    let temp_path = path.with_extension("tmp");
    tokio::fs::write(&temp_path, data).await
        .map_err(|e| anyhow::anyhow!("Failed to write metadata: {}", e))?;
    
    tokio::fs::rename(&temp_path, &path).await
        .map_err(|e| anyhow::anyhow!("Failed to rename metadata file: {}", e))?;
    
    Ok(())
}

// Upload file to Telegram Saved Messages (unencrypted for viewing in Telegram)
pub async fn upload_file(
    client_ref: Arc<Mutex<Option<Client>>>,
    file_path: &str,
    folder: &str,
    on_progress: impl Fn(u32) + Send + Sync + 'static,
) -> Result<String> {
    let path = Path::new(file_path);
    let file_name = path
        .file_name()
        .and_then(|n| n.to_str())
        .ok_or_else(|| anyhow::anyhow!("Invalid file name"))?;

    // Get file size
    let file_metadata = tokio::fs::metadata(file_path).await?;
    let file_size = file_metadata.len();

    // Get mime type
    let mime_type = mime_guess::from_path(path)
        .first_or_octet_stream()
        .to_string();

    // Get client and perform upload
    let message_id = {
        let client_guard = client_ref.lock().await;
        if let Some(ref client) = *client_guard {
            // Get self user for Saved Messages
            let me = client.get_me().await?;
            
            // Create a progress-tracking reader
            let file = tokio::fs::File::open(file_path).await?;
            let mut progress_reader = ProgressReader::new(file, file_size, on_progress);
            
            // Upload file directly to Telegram using the stream
            let uploaded_file = client.upload_stream(
                &mut progress_reader, 
                file_size as usize, 
                file_name.to_string()
            ).await?;
            
            // Send to Saved Messages (self chat) with caption
            let caption = format!("📁 {}", file_name);
            let mut input_message = grammers_client::types::InputMessage::default();
            input_message = input_message.text(&caption);
            input_message = input_message.document(uploaded_file);
            
            let peer = grammers_client::types::Peer::User(me.clone());
            let message = client.send_message(&peer, input_message).await?;
            
            // Return message ID before releasing lock
            message.id()
        } else {
            return Err(anyhow::anyhow!("Client not initialized"));
        }
    }; // Lock released here
    
    // Update metadata after releasing client lock
    // This prevents holding the lock too long and causing crashes
    let metadata_result = async {
        let mut metadata = load_metadata_copy().await?;
        metadata.files.push(FileMetadata {
            id: message_id.to_string(),
            name: file_name.to_string(),
            size: file_size,
            mime_type,
            created_at: chrono::Utc::now().timestamp(),
            folder: folder.to_string(),
            is_folder: false,
            thumbnail: None,
            message_id: Some(message_id),
            encrypted: false,
        });

        // Save updated metadata locally
        save_metadata_local(&metadata).await?;
        Ok::<(), anyhow::Error>(())
    }.await;
    
    // Log metadata save errors but don't fail the upload
    if let Err(e) = metadata_result {
        eprintln!("Warning: Failed to save metadata: {}", e);
        // Continue anyway - file is uploaded successfully
    }

    Ok(message_id.to_string())
}

// Download file from Telegram
pub async fn download_file(
    client_ref: Arc<Mutex<Option<Client>>>,
    file_id: &str,
    destination: &str,
) -> Result<String> {
    ensure_metadata_loaded().await?;
    
    let file_meta = {
        let cache = METADATA_CACHE.read().await;
        let metadata = cache.as_ref().unwrap();
        metadata.files.iter().find(|f| f.id == file_id).cloned()
    };
    
    let file_meta = file_meta.ok_or_else(|| anyhow::anyhow!("File not found"))?;

    let message_id = file_meta
        .message_id
        .ok_or_else(|| anyhow::anyhow!("No message ID for file"))?;

    // Get client
    let client_guard = client_ref.lock().await;
    if let Some(ref client) = *client_guard {
        // Get self user
        let me = client.get_me().await?;
        let peer = grammers_client::types::Peer::User(me.clone());
        
        // Get messages from Saved Messages
        let mut messages = client.iter_messages(&peer);
        
        // Find the specific message
        while let Some(message) = messages.next().await? {
            if message.id() == message_id {
                if let Some(media) = message.media() {
                    // Download media directly (no decryption needed - files are unencrypted)
                    client.download_media(&media, destination).await?;

                    // Remove macOS quarantine attributes
                    #[cfg(target_os = "macos")]
                    {
                        use std::process::Command;
                        use std::path::Path;

                        let dest_path = Path::new(destination);
                        if dest_path.exists() && dest_path.is_file() {
                            let _ = Command::new("xattr")
                                .args(&["-d", "com.apple.quarantine", destination])
                                .output();
                        }
                    }

                    return Ok(destination.to_string());
                }
            }
        }
        
        Err(anyhow::anyhow!("Message not found"))
    } else {
        Err(anyhow::anyhow!("Client not initialized"))
    }
}

// Download thumbnail from Telegram
pub async fn download_thumbnail(
    client_ref: Arc<Mutex<Option<Client>>>,
    file_id: &str,
    destination: &str,
) -> Result<Option<String>> {
    ensure_metadata_loaded().await?;
    
    // Scope the read lock
    let file_meta = {
        let cache = METADATA_CACHE.read().await;
        let metadata = cache.as_ref().unwrap();
        metadata.files.iter().find(|f| f.id == file_id).cloned()
    };

    let file_meta = file_meta.ok_or_else(|| anyhow::anyhow!("File not found"))?;

    // Only attempt download for images
    // For videos, downloading the full file as a "thumbnail" is too heavy
    if !file_meta.mime_type.starts_with("image/") {
        return Ok(None);
    }

    let message_id = file_meta
        .message_id
        .ok_or_else(|| anyhow::anyhow!("No message ID for file"))?;

    // Get client
    let client_guard = client_ref.lock().await;
    if let Some(ref client) = *client_guard {
        let me = client.get_me().await?;
        let peer = grammers_client::types::Peer::User(me.clone());
        let mut messages = client.iter_messages(&peer);
        
        while let Some(message) = messages.next().await? {
            if message.id() == message_id {
                if let Some(media) = message.media() {
                    // For images, download the media to the destination
                    // Check if destination exists first to avoid re-downloading
                    if !std::path::Path::new(destination).exists() {
                        client.download_media(&media, destination).await?;
                        
                        // Remove macOS quarantine
                        #[cfg(target_os = "macos")]
                        {
                            use std::process::Command;
                            use std::path::Path;

                            let dest_path = Path::new(destination);
                            if dest_path.exists() && dest_path.is_file() {
                                let _ = Command::new("xattr")
                                    .args(&["-d", "com.apple.quarantine", destination])
                                    .output();
                            }
                        }
                    }
                    
                    return Ok(Some(destination.to_string()));
                }
            }
        }
        Err(anyhow::anyhow!("Message not found"))
    } else {
        Err(anyhow::anyhow!("Client not initialized"))
    }
}

// List files in folder
pub async fn list_files(folder: &str) -> Result<Vec<FileMetadata>> {
    ensure_metadata_loaded().await?;
    let cache = METADATA_CACHE.read().await;
    let metadata = cache.as_ref().unwrap();
    
    Ok(metadata.files.iter()
        .filter(|f| f.folder == folder)
        .cloned()
        .collect())
}

// Create folder
pub async fn create_folder(
    _client_ref: Arc<Mutex<Option<Client>>>,
    folder_name: &str,
    parent_folder: &str,
) -> Result<String> {
    // Validate folder name
    if folder_name.trim().is_empty() {
        return Err(anyhow::anyhow!("Folder name cannot be empty"));
    }
    
    // Sanitize folder name (remove invalid characters)
    let sanitized_name = folder_name.trim().replace('/', "_").replace('\\', "_");
    if sanitized_name.is_empty() {
        return Err(anyhow::anyhow!("Invalid folder name"));
    }
    
    let full_path = if parent_folder == "/" {
        format!("/{}", sanitized_name)
    } else {
        format!("{}/{}", parent_folder.trim_end_matches('/'), sanitized_name)
    };
    
    let mut metadata = load_metadata_copy().await?;
    
    // Check if folder already exists
    if metadata.folders.contains(&full_path) {
        return Err(anyhow::anyhow!("Folder already exists"));
    }
    
    // Check if a file/folder with this name already exists in the parent folder
    let existing = metadata.files.iter().any(|f| 
        f.folder == parent_folder && f.name == sanitized_name
    );
    if existing {
        return Err(anyhow::anyhow!("A file or folder with this name already exists"));
    }
    
    metadata.folders.push(full_path.clone());
    
    // Add folder as virtual entry
    metadata.files.push(FileMetadata {
        id: format!("folder_{}", chrono::Utc::now().timestamp_nanos_opt().unwrap_or(0)),
        name: sanitized_name.clone(),
        size: 0,
        mime_type: "folder".to_string(),
        created_at: chrono::Utc::now().timestamp(),
        folder: parent_folder.to_string(),
        is_folder: true,
        thumbnail: None,
        message_id: None,
        encrypted: false,
    });
    
    save_metadata_local(&metadata).await?;
    
    Ok(full_path)
}

// Delete file
pub async fn delete_file(
    client_ref: Arc<Mutex<Option<Client>>>,
    file_id: &str,
) -> Result<bool> {
    let mut metadata = load_metadata_copy().await?;
    
    if let Some(pos) = metadata.files.iter().position(|f| f.id == file_id) {
        let file_meta = &metadata.files[pos];
        
        // Get message_id before removing from metadata
        let message_id = file_meta.message_id;
        
        // Delete the actual message from Telegram if we have a message_id
        if let Some(msg_id) = message_id {
            let client_guard = client_ref.lock().await;
            if let Some(ref client) = *client_guard {
                if let Ok(me) = client.get_me().await {
                    let peer = grammers_client::types::Peer::User(me.clone());
                    
                    // Delete the message from Telegram
                    // Note: For saved messages, we need to delete by message ID
                    // grammers-client should support delete_messages
                    let message_ids = vec![msg_id];
                    if let Err(e) = client.delete_messages(&peer, &message_ids).await {
                        eprintln!("Warning: Failed to delete message from Telegram: {:?}", e);
                        // Continue with metadata deletion even if Telegram deletion fails
                    }
                }
            }
        }
        
        // Remove from local metadata
        metadata.files.remove(pos);
        save_metadata_local(&metadata).await?;
        
        Ok(true)
    } else {
        Ok(false)
    }
}

// Get storage stats
pub async fn get_storage_stats() -> Result<StorageStats> {
    ensure_metadata_loaded().await?;
    let cache = METADATA_CACHE.read().await;
    let metadata = cache.as_ref().unwrap();
    
    let total_size: u64 = metadata.files.iter().filter(|f| !f.is_folder).map(|f| f.size).sum();
    let total_files = metadata.files.iter().filter(|f| !f.is_folder).count() as u64;
    let folder_count = metadata.folders.len() as u64;
    
    Ok(StorageStats {
        total_files,
        total_size,
        folder_count,
    })
}
