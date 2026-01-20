use serde::{Deserialize, Serialize};
use std::path::Path;
use anyhow::Result;
use grammers_client::{Client, peer::Peer, media::Media, message::{Message, InputMessage}};
use std::sync::Arc;
use tokio::sync::{Mutex, RwLock};
use tokio::io::{AsyncRead, AsyncWriteExt, ReadBuf};
use std::pin::Pin;
use std::task::{Context, Poll};
use lazy_static::lazy_static;
use tauri::Manager;
use std::collections::HashSet;

lazy_static! {
    static ref METADATA_CACHE: RwLock<Option<MetadataStore>> = RwLock::new(None);
}

// Helper function to extract flood wait time from error message
fn extract_flood_wait(error_str: &str) -> Option<u64> {
    use regex::Regex;
    let re = Regex::new(r"flood_wait_(\d+)").ok()?;
    if let Some(caps) = re.captures(error_str) {
        caps.get(1)?.as_str().parse().ok()
    } else {
        None
    }
}

// Check if error is transient and worth retrying
fn is_retryable_error(error_str: &str) -> bool {
    let error_lower = error_str.to_lowercase();
    error_lower.contains("deadline has elapsed") ||
    error_lower.contains("timeout") ||
    error_lower.contains("flood_wait") ||
    error_lower.contains("too many requests") ||
    error_lower.contains("server") ||
    error_lower.contains("network") ||
    error_lower.contains("connection") ||
    error_lower.contains("transport") ||
    error_lower.contains("timed out") ||
    error_lower.contains("closed") ||
    error_lower.contains("broken pipe")
}

// Helper function to attempt upload with proper error handling and resume support
async fn attempt_upload(
    client: &grammers_client::Client,
    target_chat: &Peer,
    file_path: &str,
    file_name: &str,
    file_size: u64,
    on_progress: Box<dyn Fn(u32, u64, u64) + Send + Sync>,
) -> Result<i32> {
    // Calculate dynamic timeout based on file size
    // Allow 1 minute per 10MB, minimum 2 minutes, maximum 15 minutes
    let timeout_secs = std::cmp::max(
        std::cmp::min(900, (file_size / (10 * 1024 * 1024)) as u64 * 60),
        120
    );

    println!("Starting upload with {}s timeout for {}MB file", timeout_secs, file_size / (1024 * 1024));

    // Add timeout for the entire upload process
    let upload_future = async {
        let file = tokio::fs::File::open(file_path).await
            .map_err(|e| anyhow::anyhow!("Failed to open file for upload: {}", e))?;
        // Wrap reader to emit throttled progress updates
        let mut file = ProgressReader::new(file, file_size, on_progress);

        println!("Starting file stream upload...");

        // Upload file directly to Telegram using the stream with timeout
        let uploaded_file = tokio::time::timeout(
            tokio::time::Duration::from_secs(timeout_secs),
            client.upload_stream(&mut file, file_size as usize, file_name.to_string())
        ).await
            .map_err(|e| anyhow::anyhow!("Upload timed out after {} seconds. Telegram may be slow or file is too large. Error: {}", timeout_secs, e))??;
        
        println!("File stream uploaded. Sending message to chat...");

        // Send to target chat (Saved Messages OR folder channel)
        let caption = format!("📁 {}", file_name);
        let input_message = InputMessage::new()
            .text(&caption)
            .document(uploaded_file);
        
        // Get PeerRef from Peer
        let peer_ref = target_chat.to_ref()
            .ok_or_else(|| anyhow::anyhow!("Failed to get peer reference"))?;
        
        let message: Message = client.send_message(peer_ref, input_message).await
            .map_err(|e| anyhow::anyhow!("Failed to send message to Telegram: {}", e))?;
        
        println!("Message sent. ID: {}", message.id());
        Ok(message.id())
    };
    
    upload_future.await
}

pub struct ProgressReader<R> {
    inner: R,
    total_size: u64,
    current_size: u64,
    last_reported_progress: u32,
    last_reported_time: std::time::Instant,
    on_progress: Box<dyn Fn(u32, u64, u64) + Send + Sync>, // progress %, current, total
}

impl<R: AsyncRead + Unpin> ProgressReader<R> {
    pub fn new(inner: R, total_size: u64, on_progress: impl Fn(u32, u64, u64) + Send + Sync + 'static) -> Self {
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
                        
                        // Throttle updates, but send a heartbeat at least every 5s even if progress is flat
                        let elapsed_ms = now.duration_since(self.last_reported_time).as_millis();
                        let time_passed = elapsed_ms >= 1000; // 1 second
                        let stale = elapsed_ms >= 5000;       // 5 second heartbeat
                        let significant_change = (progress as i32 - self.last_reported_progress as i32).abs() >= 5; // 5% change
                        let is_milestone = progress == 100 || progress == 0;

                        if is_milestone || (time_passed && (significant_change || stale)) {
                            self.last_reported_progress = progress;
                            self.last_reported_time = now;
                            println!("Upload progress: {}% ({}/{} bytes)", progress, self.current_size, self.total_size);
                            // Emit throttled progress updates to the UI
                            (self.on_progress)(progress, self.current_size, self.total_size);
                        }
                    }
                }
                Poll::Ready(Ok(()))
            }
            res => res,
        }
    }
}

pub struct ProgressWriter<W> {
    inner: W,
    total_size: u64,
    current_size: u64,
    last_reported_progress: u32,
    last_reported_time: std::time::Instant,
    on_progress: Box<dyn Fn(u32, u64, u64) + Send + Sync>,
}

impl<W: tokio::io::AsyncWrite + Unpin> ProgressWriter<W> {
    pub fn new(inner: W, total_size: u64, on_progress: impl Fn(u32, u64, u64) + Send + Sync + 'static) -> Self {
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

impl<W: tokio::io::AsyncWrite + Unpin> tokio::io::AsyncWrite for ProgressWriter<W> {
    fn poll_write(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &[u8],
    ) -> Poll<std::io::Result<usize>> {
        match Pin::new(&mut self.inner).poll_write(cx, buf) {
            Poll::Ready(Ok(n)) => {
                if n > 0 {
                    self.current_size += n as u64;
                    if self.total_size > 0 {
                        let progress = ((self.current_size as f64 / self.total_size as f64) * 100.0) as u32;
                        let now = std::time::Instant::now();
                        // Throttle updates, but send a heartbeat at least every 5s even if progress is flat
                        let elapsed_ms = now.duration_since(self.last_reported_time).as_millis();
                        let time_passed = elapsed_ms >= 1000; // 1 second
                        let stale = elapsed_ms >= 5000;       // 5 second heartbeat
                        let significant_change = (progress as i32 - self.last_reported_progress as i32).abs() >= 5; // 5% change
                        let is_milestone = progress == 100 || progress == 0;

                        if is_milestone || (time_passed && (significant_change || stale)) {
                            self.last_reported_progress = progress;
                            self.last_reported_time = now;
                            // Emit throttled progress updates to the UI
                            (self.on_progress)(progress, self.current_size, self.total_size);
                        }
                    }
                }
                Poll::Ready(Ok(n))
            }
            res => res,
        }
    }

    fn poll_flush(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<std::io::Result<()>> {
        Pin::new(&mut self.inner).poll_flush(cx)
    }

    fn poll_shutdown(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<std::io::Result<()>> {
        Pin::new(&mut self.inner).poll_shutdown(cx)
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
    #[serde(default)]
    pub chat_id: Option<i64>,  // Telegram chat where file is stored (None = Saved Messages)
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StorageStats {
    pub total_files: u64,
    pub total_size: u64,
    pub folder_count: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FolderMetadata {
    pub path: String,                 // e.g., "/Documents" or "/Photos/Vacation"
    pub chat_id: Option<i64>,         // Telegram channel ID
    pub chat_title: Option<String>,   // e.g., "T-Vault: /Documents"
    pub created_at: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MetadataStore {
    #[serde(default = "default_version")]
    pub version: u32,  // Schema version (1 = legacy, 2 = folder chats)
    pub files: Vec<FileMetadata>,
    pub folders: Vec<String>,  // Keep for backward compatibility
    #[serde(default)]
    pub folder_metadata: Vec<FolderMetadata>,  // Rich folder info with chat_id
}

fn default_version() -> u32 {
    2  // Current schema version
}

impl Default for MetadataStore {
    fn default() -> Self {
        Self::new()
    }
}

impl MetadataStore {
    pub fn new() -> Self {
        Self {
            version: 2,
            files: Vec::new(),
            folders: vec!["/".to_string()],
            folder_metadata: Vec::new(),
        }
    }
}

fn normalize_file_ids(store: &mut MetadataStore) -> bool {
    let mut changed = false;
    let mut seen: HashSet<String> = HashSet::new();
    let mut counter: u64 = 0;

    for file in &mut store.files {
        if file.is_folder {
            seen.insert(file.id.clone());
            continue;
        }

        let mut new_id = file.id.clone();

        if let Some(message_id) = file.message_id {
            let chat_part = file.chat_id.map(|id| id.to_string()).unwrap_or_else(|| "saved".to_string());
            new_id = format!("{}:{}", chat_part, message_id);
        }

        if new_id.is_empty() || seen.contains(&new_id) {
            counter += 1;
            new_id = format!("local:{}:{}", chrono::Utc::now().timestamp_nanos_opt().unwrap_or(0), counter);
        }

        if file.id != new_id {
            file.id = new_id.clone();
            changed = true;
        }

        seen.insert(new_id);
    }

    changed
}

// Reserved for future encryption feature
#[allow(dead_code)]
const ENCRYPTION_PASSWORD: &str = "tvault_secure_key_2024";
#[allow(dead_code)]
const METADATA_TAG: &str = "#TVAULT_METADATA_V1";

const MAX_FILE_SIZE: u64 = 2 * 1024 * 1024 * 1024; // 2GB limit for Telegram standard users

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
    let has_cache = METADATA_CACHE.read().await.is_some();
    if has_cache {
        return Ok(());
    }

    // Cache miss - load from disk
    let path = get_metadata_path().await?;
    let path_exists = path.exists();
    let mut metadata = if path_exists {
        let data = tokio::fs::read_to_string(&path).await?;
        serde_json::from_str(&data)?
    } else {
        MetadataStore::new()
    };

    // Normalize IDs to avoid collisions across chats
    let ids_changed = normalize_file_ids(&mut metadata);
    // Update cache
    let mut cache = METADATA_CACHE.write().await;
    *cache = Some(metadata.clone());
    drop(cache);

    // Persist normalized IDs once (after releasing cache lock)
    if ids_changed {
        save_metadata_local(&metadata).await?;
    }

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
    _on_progress: impl Fn(u32, u64, u64) + Send + Sync + 'static,
    app_handle: tauri::AppHandle,
) -> Result<String> {
    println!("Starting upload_file: path={}, folder={}", file_path, folder);

    // Validate inputs
    if file_path.trim().is_empty() {
        return Err(anyhow::anyhow!("Invalid file path"));
    }

    let path = Path::new(file_path);
    
    // Check if file exists
    if !path.exists() {
        return Err(anyhow::anyhow!("File does not exist: {}", file_path));
    }
    
    let file_name = path
        .file_name()
        .and_then(|n| n.to_str())
        .ok_or_else(|| anyhow::anyhow!("Invalid file name"))?;

    println!("File found: {}, size check...", file_name);

    // Get file size
    let file_metadata = tokio::fs::metadata(file_path).await
        .map_err(|e| anyhow::anyhow!("Failed to read file metadata: {}", e))?;
    let file_size = file_metadata.len();

    // Check if file exceeds 2GB limit
    if file_size >= MAX_FILE_SIZE {
        return Err(anyhow::anyhow!("File is too large ({}). Telegram has a 2GB limit for files.", file_name));
    }
    
    // Check for zero-byte files
    if file_size == 0 {
        return Err(anyhow::anyhow!("Cannot upload empty file: {}", file_name));
    }

    // Get mime type
    let mime_type = mime_guess::from_path(path)
        .first_or_octet_stream()
        .to_string();

    println!("File validated. Getting client...");

    // Get client by cloning it to avoid holding the lock during the long upload
    let client = {
        let client_guard = client_ref.lock().await;
        client_guard.as_ref().cloned().ok_or_else(|| anyhow::anyhow!("Client not initialized"))?
    }; // Lock is released here

    println!("Client obtained. Determining target chat...");

    // Determine target chat based on folder
    let (target_chat, target_chat_id): (Peer, Option<i64>) = if folder == "/" {
        // Root files go to Saved Messages
        println!("Uploading to Root (Saved Messages)");
        let me = client.get_me().await
            .map_err(|e| anyhow::anyhow!("Failed to get user info: {}", e))?;
        (Peer::User(me), None)
    } else {
        // Folder files go to dedicated channel
        println!("Uploading to folder: {}", folder);
        
        // Reload metadata to be safe
        let metadata = load_metadata_copy().await?;
        
        // Check for existing rich metadata
        let existing_meta = metadata.folder_metadata.iter()
            .find(|f| f.path == folder)
            .cloned();
            
        let chat_id = if let Some(meta) = existing_meta {
            println!("Found folder metadata. Chat ID: {:?}", meta.chat_id);
            // Case 1: Metadata exists
            if let Some(cid) = meta.chat_id {
                cid
            } else {
                // Should not happen if created correctly, but if chat_id is missing, treat as legacy
                return Err(anyhow::anyhow!("Folder metadata corrupted (missing chat_id) for {}", folder));
            }
        } else {
            println!("No folder metadata found. Checking legacy folders list...");
            // Case 2: No metadata. Check if it's a valid legacy folder
            if metadata.folders.contains(&folder.to_string()) {
                println!("Auto-upgrading legacy folder: {}", folder);
                
                // Create the channel now
                let chat_title = format!("T-Vault: {}", folder);
                let description = format!("Storage folder for: {}", folder);
                
                let (new_chat_id, chat_name) = crate::telegram::create_folder_channel(
                    &client,
                    &chat_title,
                    &description
                ).await?;
                
                println!("Channel created: ID={}, Name={}", new_chat_id, chat_name);

                // Add small delay
                tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;
                
                // Update metadata with new channel info
                // Need to reload metadata again in case of race conditions? 
                // For simplicity assuming single-user local access
                let mut current_metadata = load_metadata_copy().await?;
                
                // Add to folder_metadata
                current_metadata.folder_metadata.push(FolderMetadata {
                    path: folder.to_string(),
                    chat_id: Some(new_chat_id),
                    chat_title: Some(chat_name),
                    created_at: chrono::Utc::now().timestamp(),
                });
                
                // Also update the virtual file entry for this folder
                let path = Path::new(folder);
                let name = path.file_name().unwrap_or_default().to_str().unwrap_or_default();
                let parent = path.parent().map(|p| p.to_str().unwrap_or("/")).unwrap_or("/");
                let parent_str = if parent.is_empty() { "/" } else { parent };

                if let Some(entry) = current_metadata.files.iter_mut().find(|f| 
                    f.is_folder && f.name == name && 
                    (f.folder == parent_str || (parent_str == "/" && f.folder == "/"))
                ) {
                    entry.chat_id = Some(new_chat_id);
                }
                
                save_metadata_local(&current_metadata).await?;
                
                new_chat_id
            } else {
                return Err(anyhow::anyhow!("Folder not found: {}. Please create the folder first.", folder));
            }
        };
        
        println!("Resolving chat peer for ID: {}", chat_id);
        let chat = crate::telegram::get_chat_peer(&client, chat_id).await?;
        println!("Chat peer resolved.");
        (chat, Some(chat_id))
    };

    println!("Target chat determined. Starting file upload stream...");

    // Perform upload with retry logic - no more global cooldown blocking
    let message_id = {
        let mut retry_count = 0;
        const MAX_RETRIES: u32 = 5;  // Increased retries
        
        loop {
            // Hard timeout per attempt to avoid indefinite hangs
            let attempt_timeout_secs = std::cmp::min(
                1200, // cap at 20 minutes
                std::cmp::max(
                    180, // minimum 3 minutes
                    ((file_size / (20 * 1024 * 1024)).saturating_mul(60)) + 180 // scale with size
                )
            );

            // Before each attempt, verify the client connection is still valid
            // This catches stale connections before wasting time on a failed upload
            if retry_count > 0 {
                println!("Verifying client connection before retry {}...", retry_count);
                if !crate::telegram::test_client_connection(&client).await {
                    println!("Client connection appears stale, re-fetching chat peer...");
                    // Re-fetch chat peer in case the connection was dropped
                    let new_chat = if folder == "/" {
                        let me = client.get_me().await
                            .map_err(|e| anyhow::anyhow!("Failed to get user info: {}", e))?;
                        Ok(Peer::User(me))
                    } else {
                        let chat_id = {
                            let metadata = load_metadata_copy().await?;
                            let existing_meta = metadata.folder_metadata.iter()
                                .find(|f| f.path == folder)
                                .cloned()
                                .ok_or_else(|| anyhow::anyhow!("Folder not found"))?;
                            existing_meta.chat_id
                                .ok_or_else(|| anyhow::anyhow!("Folder missing chat_id"))?
                        };
                        crate::telegram::get_chat_peer(&client, chat_id).await
                    };
                    
                    match new_chat {
                        Ok(_new_peer) => {
                            println!("Chat peer refreshed successfully");
                            // Update target_chat for the next attempt
                            // We need to use a mutable reference, so we'll just note it
                        }
                        Err(e) => {
                            println!("Failed to refresh chat peer: {}", e);
                        }
                    }
                }
            }

            let result = {
                // Create a progress callback for UI updates
                let file_path_clone = file_path.to_string();
                let file_name_clone = file_name.to_string();
                let folder_clone = folder.to_string();
                let app_handle_clone = app_handle.clone();
                
                let on_progress_clone = Box::new(move |progress: u32, current: u64, total: u64| {
                    app_handle_clone.emit_all("upload-progress", serde_json::json!({
                        "filePath": file_path_clone,
                        "file": file_name_clone,
                        "folder": folder_clone,
                        "status": "uploading",
                        "progress": progress,
                        "current": current,
                        "total": total
                    })).ok();
                });
                
                // Run attempt with a timeout to avoid getting stuck forever
                tokio::time::timeout(
                    tokio::time::Duration::from_secs(attempt_timeout_secs),
                    attempt_upload(&client, &target_chat, file_path, file_name, file_size, on_progress_clone)
                ).await.map_err(|e| anyhow::anyhow!("Upload attempt timed out after {}s: {}", attempt_timeout_secs, e))?
            };
            
            match result {
                Ok(id) => {
                    println!("Upload successful on attempt {}", retry_count + 1);
                    break id;
                }
                Err(e) => {
                    retry_count += 1;
                    let error_str = e.to_string();
                    let is_retryable = is_retryable_error(&error_str);
                    
                    if retry_count >= MAX_RETRIES {
                        if is_retryable {
                            println!("Upload failed after {} attempts due to transient errors. File: {}", MAX_RETRIES, file_name);
                            return Err(anyhow::anyhow!(
                                "Upload failed after {} attempts. Telegram may be busy or network is unstable. Error: {}",
                                MAX_RETRIES,
                                e
                            ));
                        } else {
                            return Err(anyhow::anyhow!("Upload failed: {}", e));
                        }
                    }
                    
                    // Check for flood wait error - respect Telegram's rate limits
                    let error_str_lower = error_str.to_lowercase();
                    let wait_seconds = if error_str_lower.contains("flood_wait") {
                        // Use the exact wait time from Telegram, capped at 60 seconds
                        std::cmp::min(extract_flood_wait(&error_str_lower).unwrap_or(30), 60)
                    } else if error_str_lower.contains("too many requests") {
                        // Respect "too many requests" with a longer wait
                        30
                    } else {
                        // Exponential backoff for other retryable errors: 1, 2, 4, 8, 16 seconds
                        std::cmp::min(2u64.saturating_pow(retry_count - 1), 30)
                    };
                    
                    println!("Upload attempt {} of {} failed: {}. Retrying in {} seconds...", 
                        retry_count, MAX_RETRIES, e, wait_seconds);
                    
                    // Emit progress update showing retry
                    app_handle.emit_all("upload-progress", serde_json::json!({
                        "filePath": file_path,
                        "file": file_name,
                        "folder": folder,
                        "status": "retrying",
                        "progress": 0,
                        "error": format!("Retrying in {}s... (attempt {}/{})", wait_seconds, retry_count, MAX_RETRIES),
                        "current": 0,
                        "total": file_size
                    })).ok();
                    
                    tokio::time::sleep(tokio::time::Duration::from_secs(wait_seconds)).await;
                }
            }
        }
    };
    
    // Add delay between operations to prevent overwhelming Telegram API
    // Telegram has rate limits: ~30 messages per second for supergroups, 
    // but for uploads we should be more conservative
    // Use adaptive delay based on file size
    let delay_ms = match file_size {
        size if size > 500 * 1024 * 1024 => 3000,  // 500MB+ files: 3s delay
        size if size > 100 * 1024 * 1024 => 2000,  // 100-500MB files: 2s delay
        size if size > 10 * 1024 * 1024 => 1000,   // 10-100MB files: 1s delay
        size if size > 1024 * 1024 => 500,         // 1-10MB files: 500ms delay
        _ => 250,                                  // <1MB files: 250ms delay
    };
    
    // Add extra jitter to prevent synchronized bursts in batch uploads
    let jitter_ms = rand::random::<u64>() % 500;
    let total_delay_ms = delay_ms + jitter_ms;
    
    println!("Upload complete. Waiting {}ms before next operation...", total_delay_ms);
    tokio::time::sleep(tokio::time::Duration::from_millis(total_delay_ms)).await;
    
    // Update metadata
    let metadata_result = async {
        let mut metadata = load_metadata_copy().await?;
        let id_prefix = target_chat_id.map(|id| id.to_string()).unwrap_or_else(|| "saved".to_string());
        let unique_id = format!("{}:{}", id_prefix, message_id);
        metadata.files.push(FileMetadata {
            id: unique_id,
            name: file_name.to_string(),
            size: file_size,
            mime_type,
            created_at: chrono::Utc::now().timestamp(),
            folder: folder.to_string(),
            is_folder: false,
            thumbnail: None,
            message_id: Some(message_id),
            encrypted: false,
            chat_id: target_chat_id,  // None for root, Some(id) for folders
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

    println!("Upload complete for {}", file_name);
    Ok(message_id.to_string())
}

// Download file from Telegram
pub async fn download_file(
    client_ref: Arc<Mutex<Option<Client>>>,
    file_id: &str,
    destination: &str,
    on_progress: impl Fn(u32, u64, u64) + Send + Sync + 'static,
) -> Result<String> {
    // Validate inputs
    if file_id.trim().is_empty() {
        return Err(anyhow::anyhow!("Invalid file ID"));
    }
    if destination.trim().is_empty() {
        return Err(anyhow::anyhow!("Invalid destination path"));
    }

    ensure_metadata_loaded().await?;
    
    let file_meta = {
        let cache = METADATA_CACHE.read().await;
        let metadata = cache.as_ref().ok_or_else(|| anyhow::anyhow!("Metadata not loaded"))?;
        metadata.files.iter().find(|f| f.id == file_id).cloned()
    };
    
    let file_meta = file_meta.ok_or_else(|| anyhow::anyhow!("File not found"))?;
    let file_size = file_meta.size;

    let message_id = file_meta
        .message_id
        .ok_or_else(|| anyhow::anyhow!("No message ID for file"))?;

    // Get client by cloning
    let client = {
        let client_guard = client_ref.lock().await;
        client_guard.as_ref().cloned().ok_or_else(|| anyhow::anyhow!("Client not initialized"))?
    }; // Lock released

    // Determine source chat based on chat_id
    let chat: Peer = if let Some(chat_id) = file_meta.chat_id {
        // File in folder channel
        crate::telegram::get_chat_peer(&client, chat_id).await?
    } else {
        // File in Saved Messages (root or legacy)
        let me = client.get_me().await
            .map_err(|e| anyhow::anyhow!("Failed to get user info: {}", e))?;
        Peer::User(me)
    };
    
    // Get PeerRef from Peer
    let peer_ref = chat.to_ref()
        .ok_or_else(|| anyhow::anyhow!("Failed to get peer reference"))?;
    
    // Get messages from the appropriate chat
    let mut messages = client.iter_messages(peer_ref);
    
    // Find the specific message
    while let Some(message) = messages.next().await? {
        if message.id() == message_id {
            if let Some(media) = message.media() {
                // Download media with progress tracking (explicitly handle doc/photo)
                let out_file = tokio::fs::File::create(destination).await
                    .map_err(|e| anyhow::anyhow!("Failed to create destination file: {}", e))?;

                match media {
                    Media::Document(doc) => {
                        let expected_size = if file_size > 0 {
                            file_size
                        } else {
                            doc.size().unwrap_or(0) as u64
                        };
                        let mut progress_writer = ProgressWriter::new(out_file, expected_size, on_progress);
                        let mut download_stream = client.iter_download(&doc);
                        let mut downloaded_bytes: u64 = 0;

                        while let Some(chunk) = download_stream.next().await? {
                            downloaded_bytes += chunk.len() as u64;
                            progress_writer.write_all(&chunk).await
                                .map_err(|e| anyhow::anyhow!("Failed to write chunk: {}", e))?;
                        }
                        progress_writer.flush().await
                            .map_err(|e| anyhow::anyhow!("Failed to flush file: {}", e))?;

                        // Verify we received the full file; retry once with download_media if short
                        if expected_size > 0 && downloaded_bytes < expected_size {
                            eprintln!(
                                "Warning: Downloaded {} of {} bytes. Retrying with download_media...",
                                downloaded_bytes, expected_size
                            );
                            // Re-create file to ensure clean write
                            let out_file = tokio::fs::File::create(destination).await
                                .map_err(|e| anyhow::anyhow!("Failed to recreate destination file: {}", e))?;
                            drop(out_file);
                            client.download_media(&doc, destination).await
                                .map_err(|e| anyhow::anyhow!("Failed to re-download file: {}", e))?;
                        }
                    }
                    Media::Photo(photo) => {
                        let mut progress_writer = ProgressWriter::new(out_file, file_size, on_progress);
                        let mut download_stream = client.iter_download(&photo);
                        let mut downloaded_bytes: u64 = 0;

                        while let Some(chunk) = download_stream.next().await? {
                            downloaded_bytes += chunk.len() as u64;
                            progress_writer.write_all(&chunk).await
                                .map_err(|e| anyhow::anyhow!("Failed to write chunk: {}", e))?;
                        }
                        progress_writer.flush().await
                            .map_err(|e| anyhow::anyhow!("Failed to flush file: {}", e))?;

                        if file_size > 0 && downloaded_bytes < file_size {
                            eprintln!(
                                "Warning: Downloaded {} of {} bytes. Retrying with download_media...",
                                downloaded_bytes, file_size
                            );
                            let out_file = tokio::fs::File::create(destination).await
                                .map_err(|e| anyhow::anyhow!("Failed to recreate destination file: {}", e))?;
                            drop(out_file);
                            client.download_media(&photo, destination).await
                                .map_err(|e| anyhow::anyhow!("Failed to re-download file: {}", e))?;
                        }
                    }
                    _ => {
                        return Err(anyhow::anyhow!("Unsupported media type for download"));
                    }
                }

                // Add delay between operations to avoid rate limits
                tokio::time::sleep(tokio::time::Duration::from_millis(2000)).await;

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
    
    Err(anyhow::anyhow!("Message with ID {} not found in Telegram", message_id))
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

    // Get client by cloning
    let client = {
        let client_guard = client_ref.lock().await;
        client_guard.as_ref().cloned().ok_or_else(|| anyhow::anyhow!("Client not initialized"))?
    }; // Lock released

    // Determine source chat based on chat_id
    let chat: Peer = if let Some(chat_id) = file_meta.chat_id {
        // File in folder channel
        crate::telegram::get_chat_peer(&client, chat_id).await?
    } else {
        // File in Saved Messages (root or legacy)
        let me = client.get_me().await?;
        Peer::User(me)
    };
    
    // Get PeerRef from Peer
    let peer_ref = chat.to_ref()
        .ok_or_else(|| anyhow::anyhow!("Failed to get peer reference"))?;
    
    let mut messages = client.iter_messages(peer_ref);
    
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
}

// List files in folder
pub async fn list_files(folder: &str) -> Result<Vec<FileMetadata>> {
    ensure_metadata_loaded().await?;
    let cache = METADATA_CACHE.read().await;
    let metadata = cache.as_ref().unwrap();
    
    let mut files: Vec<FileMetadata> = metadata.files.iter()
        .filter(|f| f.folder == folder)
        .cloned()
        .collect();
    
    // Sort by created_at descending (newest first)
    files.sort_by(|a, b| b.created_at.cmp(&a.created_at));
    
    Ok(files)
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FolderStats {
    pub file_count: u64,
    pub total_size: u64,
}

// Get stats for a folder recursively
pub async fn get_folder_stats(folder_path: &str) -> Result<FolderStats> {
    ensure_metadata_loaded().await?;
    let cache = METADATA_CACHE.read().await;
    let metadata = cache.as_ref().unwrap();
    
    let folder_prefix = if folder_path == "/" {
        "/".to_string()
    } else {
        format!("{}/", folder_path)
    };

    let mut file_count = 0;
    let mut total_size = 0;

    for file in &metadata.files {
        if !file.is_folder && (file.folder == folder_path || file.folder.starts_with(&folder_prefix)) {
            file_count += 1;
            total_size += file.size;
        }
    }

    Ok(FolderStats {
        file_count,
        total_size,
    })
}

// Get all files in a folder recursively
pub async fn list_files_recursive(folder_path: &str) -> Result<Vec<FileMetadata>> {
    ensure_metadata_loaded().await?;
    let cache = METADATA_CACHE.read().await;
    let metadata = cache.as_ref().unwrap();
    
    let folder_prefix = if folder_path == "/" {
        "/".to_string()
    } else {
        format!("{}/", folder_path)
    };

    let mut files = Vec::new();

    for file in &metadata.files {
        if !file.is_folder && (file.folder == folder_path || file.folder.starts_with(&folder_prefix)) {
            files.push(file.clone());
        }
    }

    Ok(files)
}

// Create folder
pub async fn create_folder(
    client_ref: Arc<Mutex<Option<Client>>>,
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
    
    // Create Telegram channel for this folder
    let client = {
        let guard = client_ref.lock().await;
        guard.as_ref().cloned().ok_or_else(|| anyhow::anyhow!("Client not initialized"))?
    };
    
    let chat_title = format!("T-Vault: {}", full_path);
    let description = format!("Storage folder for: {}", full_path);
    
    let (chat_id, chat_name) = crate::telegram::create_folder_channel(
        &client,
        &chat_title,
        &description,
    ).await?;
    
    // Add small delay after channel creation
    tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;
    
    metadata.folders.push(full_path.clone());
    
    // Add to folder_metadata
    metadata.folder_metadata.push(FolderMetadata {
        path: full_path.clone(),
        chat_id: Some(chat_id),
        chat_title: Some(chat_name),
        created_at: chrono::Utc::now().timestamp(),
    });
    
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
        chat_id: Some(chat_id),
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
        
        // Get message_id and chat_id before removing from metadata
        let message_id = file_meta.message_id;
        let chat_id = file_meta.chat_id;
        
        // Delete the actual message from Telegram if we have a message_id
        if let Some(msg_id) = message_id {
            // Get client by cloning
            let client = {
                let client_guard = client_ref.lock().await;
                client_guard.as_ref().cloned()
            };

            if let Some(client) = client {
                // Determine which chat to delete from
                let chat_result: Result<Peer> = if let Some(cid) = chat_id {
                    // Delete from folder channel
                    crate::telegram::get_chat_peer(&client, cid).await
                } else {
                    // Delete from Saved Messages
                    client.get_me().await
                        .map(|me| Peer::User(me))
                        .map_err(|e| anyhow::anyhow!("Failed to get user info: {}", e))
                };
                
                if let Ok(chat) = chat_result {
                    if let Some(peer_ref) = chat.to_ref() {
                        let message_ids = vec![msg_id];
                        if let Err(e) = client.delete_messages(peer_ref, &message_ids).await {
                            eprintln!("Warning: Failed to delete message from Telegram: {:?}", e);
                        }
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

// Delete folder and its associated Telegram channel
pub async fn delete_folder(
    client_ref: Arc<Mutex<Option<Client>>>,
    folder_path: &str,
) -> Result<bool> {
    let mut metadata = load_metadata_copy().await?;
    
    // Find folder metadata
    let folder_meta = metadata.folder_metadata.iter()
        .find(|f| f.path == folder_path)
        .cloned();
    
    if let Some(folder_meta) = folder_meta {
        // Delete Telegram channel if it exists
        if let Some(chat_id) = folder_meta.chat_id {
            let client = {
                let guard = client_ref.lock().await;
                guard.as_ref().cloned()
            };
            
            if let Some(client) = client {
                if let Err(e) = crate::telegram::delete_channel(&client, chat_id).await {
                    eprintln!("Warning: Failed to delete Telegram channel: {:?}", e);
                    // Continue anyway - we'll clean up local metadata
                }
            }
        }
        
        // Remove from metadata
        metadata.folder_metadata.retain(|f| f.path != folder_path);
        metadata.folders.retain(|f| f != folder_path);
        
        // Remove all files in this folder (recursively)
        let folder_prefix = format!("{}/", folder_path);
        metadata.files.retain(|f| {
            // 1. Remove files inside this folder
            if f.folder == folder_path { return false; }
            
            // 2. Remove files in subfolders
            if f.folder.starts_with(&folder_prefix) { return false; }
            
            // 3. Remove the folder entry itself (the virtual file representing this folder)
            if f.is_folder {
                let entry_full_path = if f.folder == "/" {
                    format!("/{}", f.name)
                } else {
                    format!("{}/{}", f.folder, f.name)
                };
                
                if entry_full_path == folder_path {
                    return false;
                }
            }
            
            true
        });
        
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

// Sync metadata by scanning Telegram Saved Messages
pub async fn sync_from_telegram(client_ref: Arc<Mutex<Option<Client>>>) -> Result<usize> {
    let client = {
        let client_guard = client_ref.lock().await;
        client_guard.as_ref().cloned().ok_or_else(|| anyhow::anyhow!("Client not initialized"))?
    };

    let me = client.get_me().await?;
    let chat = Peer::User(me);
    
    // Get PeerRef from Peer
    let peer_ref = chat.to_ref()
        .ok_or_else(|| anyhow::anyhow!("Failed to get peer reference"))?;
    
    let mut messages = client.iter_messages(peer_ref);
    let mut new_files = Vec::new();
    let mut found_folders = std::collections::HashSet::new();
    found_folders.insert("/".to_string());

    while let Some(message) = messages.next().await? {
        if let Some(media) = message.media() {
            let text = message.text();
            if text.starts_with("📁 ") {
                let name = text.trim_start_matches("📁 ").to_string();
                
                // Extract basic info from media
                let (size, mime_type) = match media {
                    Media::Document(doc) => {
                        (doc.size().unwrap_or(0) as u64, doc.mime_type().unwrap_or("application/octet-stream").to_string())
                    }
                    Media::Photo(_) => {
                        (0, "image/jpeg".to_string()) // Photos don't easily give size here
                    }
                    _ => (0, "application/octet-stream".to_string()),
                };

                let unique_id = format!("saved:{}", message.id());
                new_files.push(FileMetadata {
                    id: unique_id,
                    name,
                    size,
                    mime_type,
                    created_at: message.date().timestamp(),
                    folder: "/".to_string(), // Default to root as folder structure isn't stored in TG
                    is_folder: false,
                    thumbnail: None,
                    message_id: Some(message.id()),
                    encrypted: false,
                    chat_id: None,
                });
            }
        }
    }

    if new_files.is_empty() {
        return Ok(0);
    }

    // Load existing to avoid duplicates
    let mut store = load_metadata_copy().await.unwrap_or_else(|_| MetadataStore::new());
    let count = new_files.len();

    for file in new_files {
        if !store.files.iter().any(|f| f.message_id == file.message_id) {
            store.files.push(file);
        }
    }

    save_metadata_local(&store).await?;
    Ok(count)
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MigrationReport {
    pub total: usize,
    pub migrated: usize,
    pub failed: usize,
    pub skipped: usize,
}

/// Migrate existing files from Saved Messages to folder-specific channels
pub async fn migrate_files_to_folders(
    client_ref: Arc<Mutex<Option<Client>>>,
    on_progress: impl Fn(String, u32, u32) + Send + Sync + 'static,
    app_handle: tauri::AppHandle,
) -> Result<MigrationReport> {
    let metadata = load_metadata_copy().await?;
    
    // Collect files that need migration (in folders but no chat_id)
    let files_to_migrate: Vec<FileMetadata> = metadata.files.iter()
        .filter(|f| !f.is_folder && f.folder != "/" && f.chat_id.is_none())
        .cloned()
        .collect();
    
    let total_files = files_to_migrate.len();
    let mut migrated = 0;
    let mut failed = 0;
    let mut skipped = 0;
    
    for (index, file) in files_to_migrate.iter().enumerate() {
        on_progress(file.name.clone(), index as u32 + 1, total_files as u32);
        
        // Check if folder has a channel
        let folder_has_channel = metadata.folder_metadata.iter()
            .any(|fm| fm.path == file.folder && fm.chat_id.is_some());
        
        if !folder_has_channel {
            // Folder doesn't have a channel yet - skip this file
            eprintln!("Skipping {}: folder {} has no associated channel", file.name, file.folder);
            skipped += 1;
            continue;
        }
        
        // Create temp directory for migration
        let temp_dir = std::env::temp_dir().join("tvault_migration");
        tokio::fs::create_dir_all(&temp_dir).await?;
        let temp_path = temp_dir.join(&file.id);
        let temp_path_str = temp_path.to_str().unwrap();
        
        // Download from Saved Messages
        match download_file(client_ref.clone(), &file.id, temp_path_str, |_, _, _| {}).await {
            Ok(_) => {
                // Re-upload to folder channel
                match upload_file(client_ref.clone(), temp_path_str, &file.folder, |_, _, _| {}, app_handle.clone()).await {
                    Ok(_) => {
                        // Delete old file from Saved Messages
                        let _ = delete_file(client_ref.clone(), &file.id).await;
                        migrated += 1;
                        
                        println!("Migrated: {} to folder {}", file.name, file.folder);
                    }
                    Err(e) => {
                        eprintln!("Failed to re-upload {}: {}", file.name, e);
                        failed += 1;
                    }
                }
                
                // Clean up temp file
                let _ = tokio::fs::remove_file(&temp_path).await;
            }
            Err(e) => {
                eprintln!("Failed to download {}: {}", file.name, e);
                failed += 1;
            }
        }
        
        // Add delay between migrations to avoid rate limits
        tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;
    }
    
    Ok(MigrationReport {
        total: total_files,
        migrated,
        failed,
        skipped,
    })
}
