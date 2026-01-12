use serde::{Deserialize, Serialize};
use std::path::Path;
use anyhow::Result;
// Note: Encryptor is available for future encryption feature implementation
#[allow(unused_imports)]
use crate::encryption::Encryptor;
use grammers_client::Client;
use std::sync::Arc;
use tokio::sync::Mutex;

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
const ENCRYPTION_PASSWORD: &str = "unlim_cloud_secure_key_2024";
#[allow(dead_code)]
const METADATA_TAG: &str = "#UNLIM_METADATA_V1";

async fn get_metadata_path() -> Result<std::path::PathBuf> {
    // Use app data directory instead of current directory to avoid triggering Tauri rebuilds
    let data_dir = directories::ProjectDirs::from("com", "unlimcloud", "unlim-cloud")
        .ok_or_else(|| anyhow::anyhow!("Failed to get data directory"))?
        .data_dir()
        .to_path_buf();
    
    // Create directory if it doesn't exist
    tokio::fs::create_dir_all(&data_dir).await?;
    
    Ok(data_dir.join("metadata.json"))
}

async fn load_metadata_local() -> Result<MetadataStore> {
    let path = get_metadata_path().await?;
    if path.exists() {
        let data = tokio::fs::read_to_string(&path).await?;
        Ok(serde_json::from_str(&data)?)
    } else {
        Ok(MetadataStore::new())
    }
}

async fn save_metadata_local(store: &MetadataStore) -> Result<()> {
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
            
            // Upload file directly to Telegram (unencrypted)
            let uploaded_file = client.upload_file(path).await?;
            
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
        let mut metadata = load_metadata_local().await?;
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
    let metadata = load_metadata_local().await?;
    
    let file_meta = metadata
        .files
        .iter()
        .find(|f| f.id == file_id)
        .ok_or_else(|| anyhow::anyhow!("File not found"))?;

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

                    // Remove macOS quarantine attributes to prevent "certification issues"
                    // This allows the downloaded file to be opened without security warnings
                    #[cfg(target_os = "macos")]
                    {
                        use std::process::Command;
                        use std::path::Path;

                        // Ensure the destination path exists and is a valid file
                        let dest_path = Path::new(destination);
                        if dest_path.exists() && dest_path.is_file() {
                            // Remove quarantine attribute that macOS adds to downloaded files
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

// List files in folder
pub async fn list_files(folder: &str) -> Result<Vec<FileMetadata>> {
    let metadata = load_metadata_local().await?;
    Ok(metadata.files.into_iter().filter(|f| f.folder == folder).collect())
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
    
    let mut metadata = load_metadata_local().await?;
    
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
    let mut metadata = load_metadata_local().await?;
    
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
    let metadata = load_metadata_local().await?;
    
    let total_size: u64 = metadata.files.iter().filter(|f| !f.is_folder).map(|f| f.size).sum();
    let total_files = metadata.files.iter().filter(|f| !f.is_folder).count() as u64;
    let folder_count = metadata.folders.len() as u64;
    
    Ok(StorageStats {
        total_files,
        total_size,
        folder_count,
    })
}
