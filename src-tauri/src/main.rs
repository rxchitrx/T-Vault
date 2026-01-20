// Prevents additional console window on Windows in release
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod telegram;
mod storage;
mod encryption;
mod api_keys;

use tokio::sync::Mutex;
use tauri::Manager;

// Load environment variables from .env file
fn init_env() {
    // Try to load .env file, but don't fail if it doesn't exist
    // Environment variables can also be set directly
    dotenv::dotenv().ok();
}

struct AppState {
    telegram_client: Mutex<Option<telegram::TelegramClient>>,
}

#[tauri::command]
async fn telegram_login(
    phone: String,
    state: tauri::State<'_, AppState>,
) -> Result<String, String> {
    let mut client_guard = state.telegram_client.lock().await;
    
    // Check if client already exists and is authenticated
    if let Some(ref client) = *client_guard {
        if client.is_authenticated().await.unwrap_or(false) {
            return Ok("Already authenticated!".to_string());
        }
    }
    
    // Create new client if needed
    if client_guard.is_none() {
        let client = telegram::TelegramClient::new()
            .await
            .map_err(|e| e.to_string())?;
        *client_guard = Some(client);
    }
    
    // Send code
    if let Some(ref mut client) = *client_guard {
        client
            .send_code(&phone)
            .await
            .map_err(|e| e.to_string())?;
    }
    
    Ok("Verification code sent! Check your Telegram app for the code.".to_string())
}

#[tauri::command]
async fn telegram_verify_code(
    phone: String,
    code: String,
    state: tauri::State<'_, AppState>,
) -> Result<bool, String> {
    let mut client_guard = state.telegram_client.lock().await;
    
    if let Some(client) = client_guard.as_mut() {
        // Add timeout wrapper
        let verify_future = client.verify_code(&phone, &code);
        let timeout_future = tokio::time::sleep(tokio::time::Duration::from_secs(30));
        
        tokio::select! {
            result = verify_future => {
                result.map_err(|e| {
                    eprintln!("Verify code error: {}", e);
                    e.to_string()
                })?;
                Ok(true)
            }
            _ = timeout_future => {
                Err("Verification timed out. Please try requesting a new code.".to_string())
            }
        }
    } else {
        Err("No active login session. Please request a code first.".to_string())
    }
}

#[tauri::command]
async fn telegram_check_auth(state: tauri::State<'_, AppState>) -> Result<bool, String> {
    let client_guard = state.telegram_client.lock().await;
    
    if let Some(client) = client_guard.as_ref() {
        client.is_authenticated().await.map_err(|e| e.to_string())
    } else {
        Ok(false)
    }
}

#[tauri::command]
async fn check_api_keys_configured() -> Result<bool, String> {
    Ok(api_keys::ApiKeys::exists().await)
}

#[tauri::command]
async fn upload_file(
    file_path: String,
    folder: String,
    state: tauri::State<'_, AppState>,
    app_handle: tauri::AppHandle,
) -> Result<String, String> {
    // Validate inputs
    if file_path.trim().is_empty() {
        return Err("Invalid file path".to_string());
    }
    
    let file_name = std::path::Path::new(&file_path)
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("file");
    
    // Emit upload start event
    app_handle.emit_all("upload-progress", serde_json::json!({
        "filePath": file_path,
        "file": file_name,
        "folder": folder,
        "status": "uploading",
        "progress": 0
    })).ok();
    
    // Get client reference (clone Arc to avoid holding lock)
    let client_ref = {
        let client_guard = state.telegram_client.lock().await;
        if let Some(ref client) = *client_guard {
            client.get_client_ref()
        } else {
            app_handle.emit_all("upload-progress", serde_json::json!({
                "filePath": file_path,
                "file": file_name,
                "status": "error",
                "error": "Not authenticated",
                "progress": 0
            })).ok();
            return Err("Not authenticated".to_string());
        }
    }; // Lock released here
    
    // Emit progress: reading file
    app_handle.emit_all("upload-progress", serde_json::json!({
        "filePath": file_path,
        "file": file_name,
        "folder": folder,
        "status": "reading",
        "progress": 5
    })).ok();
    
    // Perform upload (client_ref is Arc, so no lock needed)
    let app_handle_clone = app_handle.clone();
    let file_name_clone = file_name.to_string();
    
    let file_path_clone = file_path.clone();
    let result = storage::upload_file(client_ref, &file_path, &folder, move |progress, current, total| {
        app_handle_clone.emit_all("upload-progress", serde_json::json!({
            "filePath": file_path_clone,
            "file": file_name_clone,
            "status": "uploading",
            "progress": progress,
            "current": current,
            "total": total
        })).ok();
    }, app_handle.clone()).await;
    
    // Emit result after upload completes
    match &result {
        Ok(_) => {
            // Emit success - delay slightly to ensure metadata is saved
            tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
            app_handle.emit_all("upload-progress", serde_json::json!({
                "filePath": file_path,
                "file": file_name,
                "folder": folder,
                "status": "completed",
                "progress": 100
            })).ok();
        }
        Err(e) => {
            // Emit error
            app_handle.emit_all("upload-progress", serde_json::json!({
                "filePath": file_path,
                "file": file_name,
                "folder": folder,
                "status": "error",
                "error": e.to_string(),
                "progress": 0
            })).ok();
        }
    }
    
    result.map_err(|e| e.to_string())
}

#[tauri::command]
async fn download_file(
    file_id: String,
    destination: String,
    state: tauri::State<'_, AppState>,
    app_handle: tauri::AppHandle,
) -> Result<String, String> {
    // Validate inputs
    if file_id.trim().is_empty() {
        return Err("Invalid file ID".to_string());
    }
    if destination.trim().is_empty() {
        return Err("Invalid destination path".to_string());
    }

    // Get file name from destination path instead of recursive scan
    let file_name = std::path::Path::new(&destination)
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("file")
        .to_string();

    let client_ref = {
        let client_guard = state.telegram_client.lock().await;
        if let Some(ref client) = *client_guard {
            client.get_client_ref()
        } else {
            app_handle.emit_all("download-progress", serde_json::json!({
                "fileId": file_id,
                "file": "file",
                "status": "error",
                "error": "Not authenticated",
                "progress": 0
            })).ok();
            return Err("Not authenticated".to_string());
        }
    }; // Lock released here

    let app_handle_clone = app_handle.clone();
    let file_id_clone = file_id.clone();
    let file_name_clone = file_name.clone();

    let result = storage::download_file(client_ref, &file_id, &destination, move |progress, current, total| {
        app_handle_clone.emit_all("download-progress", serde_json::json!({
            "fileId": file_id_clone,
            "file": file_name_clone,
            "status": "downloading",
            "progress": progress,
            "current": current,
            "total": total
        })).ok();
    }).await;

    match &result {
        Ok(_) => {
            tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
            app_handle.emit_all("download-progress", serde_json::json!({
                "fileId": file_id,
                "file": file_name,
                "status": "completed",
                "progress": 100
            })).ok();
        }
        Err(e) => {
            app_handle.emit_all("download-progress", serde_json::json!({
                "fileId": file_id,
                "file": file_name,
                "status": "error",
                "error": e.to_string(),
                "progress": 0
            })).ok();
        }
    }

    result.map_err(|e| e.to_string())
}

#[tauri::command]
async fn download_thumbnail(
    file_id: String,
    destination: String,
    state: tauri::State<'_, AppState>,
) -> Result<Option<String>, String> {
    let client_ref = {
        let client_guard = state.telegram_client.lock().await;
        if let Some(ref client) = *client_guard {
            client.get_client_ref()
        } else {
            return Err("Not authenticated".to_string());
        }
    }; // Lock released here

    storage::download_thumbnail(client_ref, &file_id, &destination)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
async fn list_files(
    folder: String,
    _state: tauri::State<'_, AppState>,
) -> Result<Vec<storage::FileMetadata>, String> {
    storage::list_files(&folder)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
async fn get_folder_stats(
    folder_path: String,
) -> Result<storage::FolderStats, String> {
    storage::get_folder_stats(&folder_path)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
async fn list_files_recursive(
    folder_path: String,
) -> Result<Vec<storage::FileMetadata>, String> {
    storage::list_files_recursive(&folder_path)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
async fn create_folder(
    folder_name: String,
    parent_folder: String,
    state: tauri::State<'_, AppState>,
) -> Result<String, String> {
    let client_ref = {
        let client_guard = state.telegram_client.lock().await;
        if let Some(ref client) = *client_guard {
            client.get_client_ref()
        } else {
            return Err("Not authenticated".to_string());
        }
    }; // Lock released
    
    let result = storage::create_folder(client_ref, &folder_name, &parent_folder).await;
    
    match &result {
        Ok(path) => Ok(path.clone()),
        Err(e) => Err(e.to_string()),
    }
}

#[tauri::command]
async fn delete_file(
    file_id: String,
    state: tauri::State<'_, AppState>,
) -> Result<bool, String> {
    let client_ref = {
        let client_guard = state.telegram_client.lock().await;
        if let Some(ref client) = *client_guard {
            client.get_client_ref()
        } else {
            return Err("Not authenticated".to_string());
        }
    }; // Lock released here

    storage::delete_file(client_ref, &file_id)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
async fn get_storage_stats(
    _state: tauri::State<'_, AppState>,
) -> Result<storage::StorageStats, String> {
    storage::get_storage_stats()
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
async fn sync_metadata(state: tauri::State<'_, AppState>) -> Result<usize, String> {
    let client_ref = {
        let client_guard = state.telegram_client.lock().await;
        if let Some(ref client) = *client_guard {
            client.get_client_ref()
        } else {
            return Err("Not authenticated".to_string());
        }
    };
    
    storage::sync_from_telegram(client_ref)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
async fn delete_folder(
    folder_path: String,
    state: tauri::State<'_, AppState>,
) -> Result<bool, String> {
    let client_ref = {
        let client_guard = state.telegram_client.lock().await;
        if let Some(ref client) = *client_guard {
            client.get_client_ref()
        } else {
            return Err("Not authenticated".to_string());
        }
    };
    
    storage::delete_folder(client_ref, &folder_path)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
async fn migrate_files_to_folders(
    state: tauri::State<'_, AppState>,
    app_handle: tauri::AppHandle,
) -> Result<storage::MigrationReport, String> {
    let client_ref = {
        let client_guard = state.telegram_client.lock().await;
        if let Some(ref client) = *client_guard {
            client.get_client_ref()
        } else {
            return Err("Not authenticated".to_string());
        }
    };
    
    let app_handle_clone = app_handle.clone();
    storage::migrate_files_to_folders(client_ref, move |file_name, current, total| {
        app_handle_clone.emit_all("migration-progress", serde_json::json!({
            "file": file_name,
            "current": current,
            "total": total,
            "progress": (current as f64 / total as f64 * 100.0) as u32,
        })).ok();
    }, app_handle.clone()).await
    .map_err(|e| e.to_string())
}

#[tauri::command]
async fn save_api_keys(api_id: i32, api_hash: String) -> Result<(), String> {
    // Validate the API keys by attempting to use them
    // This ensures the keys are correct before saving
    match telegram::TelegramClient::validate_credentials(api_id, &api_hash).await {
        Ok(_) => {
            // Keys are valid, save them
            let keys = api_keys::ApiKeys {
                api_id,
                api_hash,
            };
            keys.save().await.map_err(|e| e.to_string())?;
            Ok(())
        }
        Err(e) => {
            // Validation failed - keys are invalid
            Err(format!("Invalid API credentials: {}. Please check your API ID and API Hash from https://my.telegram.org/apps", e))
        }
    }
}

#[tauri::command]
async fn initialize_client(state: tauri::State<'_, AppState>) -> Result<bool, String> {
    // Check if we already have a client
    let mut client_guard = state.telegram_client.lock().await;
    
    if client_guard.is_none() {
        // Try to create client with existing session
        match telegram::TelegramClient::new().await {
            Ok(client) => {
                // Check if already authenticated
                let is_auth = client.is_authenticated().await.unwrap_or(false);
                *client_guard = Some(client);
                return Ok(is_auth);
            }
            Err(e) => {
                // Failed to create client, might need to login
                return Err(format!("Failed to initialize: {}", e));
            }
        }
    } else {
        // Client exists, check auth
        if let Some(ref client) = *client_guard {
            return Ok(client.is_authenticated().await.unwrap_or(false));
        }
    }
    
    Ok(false)
}

fn main() {
    init_env();
    
    // Create a custom runtime with a larger stack size to prevent stack overflow
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .thread_stack_size(4 * 1024 * 1024) // 4MB stack size
        .build()
        .unwrap();

    runtime.block_on(async {
        tauri::Builder::default()
            .manage(AppState {
                telegram_client: Mutex::new(None),
            })
            .invoke_handler(tauri::generate_handler![
                check_api_keys_configured,
                save_api_keys,
                initialize_client,
                telegram_login,
                telegram_verify_code,
                telegram_check_auth,
                upload_file,
                download_file,
                download_thumbnail,
                list_files,
                get_folder_stats,
                list_files_recursive,
                create_folder,
                delete_file,
                delete_folder,
                get_storage_stats,
                sync_metadata,
                migrate_files_to_folders,
            ])
            .run(tauri::generate_context!())
            .expect("error while running tauri application");
    });
}
