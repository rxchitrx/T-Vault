use grammers_client::{Client, SignInError, client::LoginToken};
use grammers_client::peer::{User, Peer};
use grammers_session::storages::SqliteSession;
use grammers_mtsender::{SenderPool, SenderPoolHandle};
use anyhow::{Result, Context};
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::Mutex;

use crate::api_keys::ApiKeys;

// Load API credentials from stored config file or environment variables (fallback)
async fn get_api_id() -> Result<i32> {
    // First try to load from stored config file
    if let Some(keys) = ApiKeys::load().await? {
        return Ok(keys.api_id);
    }
    
    // Fallback to environment variable (for backward compatibility)
    std::env::var("TELEGRAM_API_ID")
        .context("Telegram API credentials not configured. Please set them up in the app.")?
        .parse::<i32>()
        .context("TELEGRAM_API_ID must be a valid integer")
}

async fn get_api_hash() -> Result<String> {
    // First try to load from stored config file
    if let Some(keys) = ApiKeys::load().await? {
        return Ok(keys.api_hash);
    }
    
    // Fallback to environment variable (for backward compatibility)
    std::env::var("TELEGRAM_API_HASH")
        .context("Telegram API credentials not configured. Please set them up in the app.")
}

pub struct TelegramClient {
    client: Arc<Mutex<Option<Client>>>,
    // Kept for potential future use in connection management
    #[allow(dead_code)]
    pool_handle: Arc<Mutex<Option<SenderPoolHandle>>>,
    login_token: Arc<Mutex<Option<LoginToken>>>,
    // Kept for reference, may be used for session management in future
    #[allow(dead_code)]
    session_file: PathBuf,
    phone: String,
}

impl TelegramClient {
    // Validate API credentials by attempting to create a client and make a test call
    pub async fn validate_credentials(api_id: i32, api_hash: &str) -> Result<()> {
        let data_dir = directories::ProjectDirs::from("com", "tvault", "t-vault")
            .ok_or_else(|| anyhow::anyhow!("Failed to get data directory"))?
            .data_dir()
            .to_path_buf();
        
        tokio::fs::create_dir_all(&data_dir).await?;
        // Use a temporary session file for validation
        let temp_session_file = data_dir.join("temp_validation_session.session");
        
        // Remove temp session if it exists
        let _ = tokio::fs::remove_file(&temp_session_file).await;
        
        // Create session using SqliteSession for persistence
        let session: Arc<SqliteSession> = Arc::new(
            SqliteSession::open(temp_session_file.to_str().ok_or_else(|| anyhow::anyhow!("Invalid session path"))?)?
        );

        // Create sender pool with provided API ID
        let pool = SenderPool::new(Arc::clone(&session), api_id);
        
        // Create client BEFORE moving runner
        let client = Client::new(&pool);
        
        // Now start the pool runner in background
        let runner = pool.runner;
        let runner_handle = tokio::spawn(async move {
            runner.run().await;
        });

        // Give the runner a moment to start
        tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;

        // Try to make a test API call - attempt to request a login code with a dummy phone
        // This will validate that the API ID and hash are correct
        // We use a clearly invalid phone number so we don't actually send anything
        let test_phone = "+0000000000";
        match client.request_login_code(test_phone, api_hash).await {
            Ok(_) => {
                // This shouldn't happen with an invalid phone, but if it does, keys are valid
                runner_handle.abort();
                // Clean up temp session
                let _ = tokio::fs::remove_file(&temp_session_file).await;
                Ok(())
            }
            Err(e) => {
                runner_handle.abort();
                // Clean up temp session
                let _ = tokio::fs::remove_file(&temp_session_file).await;
                
                // Check the error - if it's about invalid API credentials, fail
                let error_str = format!("{:?}", e);
                if error_str.contains("API_ID") || error_str.contains("API_HASH") || 
                   error_str.contains("invalid") || error_str.contains("401") {
                    return Err(anyhow::anyhow!("Invalid API credentials. Please check your API ID and API Hash."));
                }
                
                // Other errors (like phone number validation) are fine - it means the API keys worked
                // The API accepted our request and rejected it for phone-related reasons, not credential reasons
                Ok(())
            }
        }
    }

    pub async fn new() -> Result<Self> {
        // Use app data directory instead of current directory to avoid triggering Tauri rebuilds
        let data_dir = directories::ProjectDirs::from("com", "tvault", "t-vault")
            .ok_or_else(|| anyhow::anyhow!("Failed to get data directory"))?
            .data_dir()
            .to_path_buf();
        
        tokio::fs::create_dir_all(&data_dir).await?;
        let session_file = data_dir.join("telegram_session.session");
        
        // Create session using SqliteSession for persistence
        let session: Arc<SqliteSession> = Arc::new(
            SqliteSession::open(session_file.to_str().ok_or_else(|| anyhow::anyhow!("Invalid session path"))?)?
        );

        // Get API credentials from stored config or environment
        let api_id = get_api_id().await?;
        
        // Create sender pool
        let pool = SenderPool::new(Arc::clone(&session), api_id);
        let pool_handle = pool.handle.clone();
        
        // Create client BEFORE moving runner
        let client = Client::new(&pool);
        
        // Now start the pool runner in background
        let runner = pool.runner;
        tokio::spawn(async move {
            runner.run().await;
        });

        Ok(Self {
            client: Arc::new(Mutex::new(Some(client))),
            pool_handle: Arc::new(Mutex::new(Some(pool_handle))),
            login_token: Arc::new(Mutex::new(None)),
            session_file,
            phone: String::new(),
        })
    }

    pub async fn send_code(&mut self, phone: &str) -> Result<()> {
        self.phone = phone.to_string();
        
        // Clear any existing token first
        let mut token_guard = self.login_token.lock().await;
        *token_guard = None;
        drop(token_guard);
        
        let client_guard = self.client.lock().await;
        if let Some(ref client) = *client_guard {
            // Check if already authorized
            if client.is_authorized().await? {
                // Already authenticated, clear token and return
                let mut token_guard = self.login_token.lock().await;
                *token_guard = None;
                return Ok(());
            }
            
            // Get API hash from stored config or environment
            let api_hash = get_api_hash().await?;
            
            // Request login code
            let token = client.request_login_code(phone, &api_hash).await?;
            
            // Store token
            let mut token_guard = self.login_token.lock().await;
            *token_guard = Some(token);
        }
        
        Ok(())
    }

    pub async fn verify_code(&mut self, _phone: &str, code: &str) -> Result<()> {
        // Get token first
        let token = {
            let mut token_guard: tokio::sync::MutexGuard<'_, Option<LoginToken>> = self.login_token.lock().await;
            token_guard.take()
        };
        
        if let Some(token) = token {
            // Clone Arc before locking to avoid holding lock during async operation
            let client_arc = self.client.clone();
            
            // Perform sign_in
            let result = {
                let client_guard = client_arc.lock().await;
                if let Some(ref client) = *client_guard {
                    client.sign_in(&token, code).await
                } else {
                    return Err(anyhow::anyhow!("Client not available"));
                }
            };
            
            match result {
                Ok(_user) => {
                    // Clear token after successful login
                    let mut token_guard = self.login_token.lock().await;
                    *token_guard = None;
                    Ok(())
                }
                Err(SignInError::PasswordRequired(_)) => {
                    Err(anyhow::anyhow!("2FA password required - please disable 2FA temporarily"))
                }
                Err(e) => {
                    eprintln!("Sign in error: {:?}", e);
                    Err(anyhow::anyhow!("Sign in failed: {:?}", e))
                }
            }
        } else {
            Err(anyhow::anyhow!("No code request in progress. Please request a new code first."))
        }
    }

    pub async fn is_authenticated(&self) -> Result<bool> {
        let client_guard = self.client.lock().await;
        if let Some(ref client) = *client_guard {
            Ok(client.is_authorized().await?)
        } else {
            Ok(false)
        }
    }

    // Get client reference for storage operations
    pub fn get_client_ref(&self) -> Arc<Mutex<Option<Client>>> {
        self.client.clone()
    }

    // Get self user - available for future features (e.g., displaying user info in UI)
    #[allow(dead_code)]
    pub async fn get_me(&self) -> Result<User> {
        let client_guard = self.client.lock().await;
        if let Some(ref client) = *client_guard {
            client.get_me().await.map_err(|e| anyhow::anyhow!("Failed to get user: {:?}", e))
        } else {
            Err(anyhow::anyhow!("Client not initialized"))
        }
    }
}

// Channel management functions for folder-based storage
/// Create a private Telegram channel for a folder
pub async fn create_folder_channel(
    client: &Client,
    title: &str,
    description: &str,
) -> Result<(i64, String)> {
    use grammers_tl_types as tl;

    // Create channel using raw TL request
    let request = tl::functions::channels::CreateChannel {
        broadcast: true,    // Private channel (not group)
        megagroup: false,   // Not a supergroup
        title: title.to_string(),
        about: description.to_string(),
        geo_point: None,
        address: None,
        for_import: false,
        forum: false,
        ttl_period: None,
    };
    
    let updates = client.invoke(&request).await
        .map_err(|e| anyhow::anyhow!("Failed to create channel: {:?}", e))?;
    
    // Extract channel from updates
    let channel = match updates {
        tl::enums::Updates::Updates(u) => {
            // Find the channel in chats
            u.chats.into_iter()
                .find_map(|chat| match chat {
                    tl::enums::Chat::Channel(c) => Some(c),
                    _ => None,
                })
                .ok_or_else(|| anyhow::anyhow!("Channel not found in updates"))?
        }
        _ => return Err(anyhow::anyhow!("Unexpected updates response")),
    };
    
    let chat_id = channel.id;
    let chat_title = channel.title.clone();
    
    Ok((chat_id, chat_title))
}

/// Delete a Telegram channel
pub async fn delete_channel(
    client: &Client,
    chat_id: i64,
) -> Result<()> {
    use grammers_tl_types as tl;
    
    // First, we need to get the channel's access hash
    // For now, we'll use the dialogs to find the channel
    let mut dialogs = client.iter_dialogs();
    let mut channel_input: Option<tl::enums::InputChannel> = None;
    
    while let Some(dialog) = dialogs.next().await
        .map_err(|e| anyhow::anyhow!("Failed to iterate dialogs: {:?}", e))? {
        if let Peer::Channel(c) = &dialog.peer {
            // Compare raw channel id directly
            if c.raw.id == chat_id {
                // Found the channel, get its InputChannel
                channel_input = Some(tl::enums::InputChannel::Channel(tl::types::InputChannel {
                    channel_id: c.raw.id,
                    access_hash: c.raw.access_hash.unwrap_or(0),
                }));
                break;
            }
        }
    }
    
    let channel_input = channel_input
        .ok_or_else(|| anyhow::anyhow!("Channel not found in dialogs"))?;
    
    // Delete the channel
    let request = tl::functions::channels::DeleteChannel {
        channel: channel_input,
    };
    
    client.invoke(&request).await
        .map_err(|e| anyhow::anyhow!("Failed to delete channel: {:?}", e))?;
    
    Ok(())
}

/// Get Peer from chat_id for sending messages
pub async fn get_chat_peer(
    client: &Client,
    chat_id: i64,
) -> Result<Peer> {
    println!("Debug: searching for chat_id: {}", chat_id);

    // Search through dialogs but with a reasonable limit to prevent hanging
    let mut dialogs = client.iter_dialogs();
    let mut count = 0;
    const MAX_DIALOGS_TO_SEARCH: usize = 50; // Reduced limit to prevent hanging
    
    while let Some(dialog) = dialogs.next().await
        .map_err(|e| anyhow::anyhow!("Failed to iterate dialogs: {:?}", e))? {
        
        count += 1;
        if count > MAX_DIALOGS_TO_SEARCH {
            println!("Debug: Stopped search after {} dialogs to prevent hanging", count);
            break;
        }
        
        if let Peer::Channel(channel) = &dialog.peer {
            // Compare raw channel id directly
            if channel.raw.id == chat_id {
                println!("Debug: Found chat in dialogs at index {}", count);
                return Ok(dialog.peer.clone());
            }
        }
    }
    
    println!("Debug: Chat not found after scanning {} dialogs", count);
    Err(anyhow::anyhow!("Chat with ID {} not found. The channel may not exist or you may not have access.", chat_id))
}

/// Test if a client connection is still valid by making a lightweight API call
pub async fn test_client_connection(client: &Client) -> bool {
    // Use get_me which is a lightweight API call
    match tokio::time::timeout(
        tokio::time::Duration::from_secs(10),
        client.get_me()
    ).await {
        Ok(Ok(_)) => {
            println!("Client connection verified");
            true
        }
        Ok(Err(e)) => {
            println!("Client connection test failed: {:?}", e);
            false
        }
        Err(_) => {
            println!("Client connection test timed out");
            false
        }
    }
}
