# T-Vault Architecture

This document explains how T-Vault works under the hood.

## Overview

T-Vault is a desktop application that uses Telegram as a storage backend. It's built with:

- **Frontend**: React + TypeScript + Tailwind CSS
- **Backend**: Rust + Tauri
- **Storage**: Telegram API (grammers library)
- **Encryption**: AES-256-GCM (optional)

## Architecture Diagram

```
┌─────────────────────────────────────────────────────────┐
│                    React Frontend                        │
│  ┌──────────┐  ┌──────────┐  ┌──────────┐  ┌─────────┐ │
│  │ Login UI │  │  Files   │  │ Gallery  │  │Settings │ │
│  └──────────┘  └──────────┘  └──────────┘  └─────────┘ │
└─────────────────────────┬───────────────────────────────┘
                          │ Tauri IPC
┌─────────────────────────▼───────────────────────────────┐
│                    Rust Backend                          │
│  ┌──────────┐  ┌──────────┐  ┌──────────┐  ┌─────────┐ │
│  │Telegram  │  │ Storage  │  │Encryption│  │  File   │ │
│  │  Client  │  │ Manager  │  │  Engine  │  │Chunking │ │
│  └──────────┘  └──────────┘  └──────────┘  └─────────┘ │
└─────────────────────────┬───────────────────────────────┘
                          │ Telegram MTProto
┌─────────────────────────▼───────────────────────────────┐
│                  Telegram Servers                        │
│         (Your "Saved Messages" or Private Channel)       │
└──────────────────────────────────────────────────────────┘
```

## How File Storage Works

### 1. File Upload Process

```
User selects file
    ↓
Read file from disk
    ↓
[Optional] Encrypt with AES-256-GCM
    ↓
Split into chunks (if > 2GB)
    ↓
Upload each chunk to Telegram
    ↓
Store metadata JSON with file info
    ↓
Update local metadata cache
```

### 2. File Download Process

```
User requests file
    ↓
Fetch metadata from Telegram
    ↓
Download all chunks
    ↓
Combine chunks
    ↓
[Optional] Decrypt
    ↓
Save to disk
```

### 3. Folder Organization

Folders are virtual - they don't exist on Telegram. Instead:

- A JSON metadata file is stored in Telegram "Saved Messages"
- This JSON contains the folder structure:

```json
{
  "version": "1.0",
  "folders": ["/", "/Documents", "/Photos", "/Videos"],
  "files": [
    {
      "id": "msg_12345",
      "name": "photo.jpg",
      "size": 2048576,
      "folder": "/Photos",
      "mime_type": "image/jpeg",
      "created_at": 1234567890,
      "chunks": ["msg_12345"],
      "encrypted": false
    }
  ]
}
```

## Components

### Frontend Components

1. **App.tsx**: Main app, handles auth state
2. **LoginScreen.tsx**: Telegram authentication UI
3. **Dashboard.tsx**: Main layout after login
4. **FileManager.tsx**: File browsing and management
5. **Gallery.tsx**: Image/video gallery view
6. **Settings.tsx**: App configuration
7. **Sidebar.tsx**: Navigation menu
8. **StorageStats.tsx**: Display storage usage

### Backend Modules

1. **main.rs**: Tauri entry point, command handlers
2. **telegram.rs**: Telegram API integration
3. **storage.rs**: File storage logic
4. **encryption.rs**: AES-256-GCM encryption

## Data Flow

### Authentication

```
Frontend              Backend              Telegram
   │                     │                     │
   ├─ Enter phone ──────▶│                     │
   │                     ├─ Request code ─────▶│
   │                     │◀─── Send code ──────┤
   │◀─ Show code input ──┤                     │
   ├─ Enter code ────────▶│                     │
   │                     ├─ Verify code ───────▶│
   │                     │◀─── Session token ───┤
   │                     ├─ Save session       │
   │◀─ Login success ────┤                     │
```

### File Upload

```
Frontend              Backend              Telegram
   │                     │                     │
   ├─ Select file ───────▶│                     │
   │                     ├─ Read file         │
   │                     ├─ Encrypt (optional)│
   │                     ├─ Create chunks     │
   │                     ├─ Upload chunk 1 ───▶│
   │                     │◀─── Message ID ─────┤
   │                     ├─ Upload chunk 2 ───▶│
   │                     │◀─── Message ID ─────┤
   │                     ├─ Update metadata ──▶│
   │◀─ Upload complete ──┤                     │
```

## Security Considerations

### What's Secure

- **Authentication**: Uses Telegram's official MTProto protocol
- **Session Storage**: Sessions encrypted locally
- **Optional E2E**: Files can be encrypted before upload
- **No Third Parties**: Direct communication with Telegram

### What's Not Secure

- **Telegram Access**: Telegram can access unencrypted files
- **Metadata**: File names, sizes visible to Telegram (unless encrypted)
- **Session Files**: If someone gets your session file, they can access your account

### Best Practices

1. Enable E2E encryption for sensitive files
2. Keep session files secure (stored in app data directory)
3. Use a strong encryption password
4. Don't share API credentials
5. Log out when done on shared computers

## Performance Optimizations

1. **Chunking**: Files split into optimal chunks for Telegram's API
2. **Caching**: Metadata cached locally to reduce API calls
3. **Lazy Loading**: Gallery images loaded on-demand
4. **Parallel Uploads**: Multiple chunks uploaded simultaneously
5. **Compression**: Optional compression before upload

## Telegram API Integration

### Using grammers-client

The app uses the `grammers-client` library for Telegram API:

```rust
// Initialize client
let client = Client::connect(Config {
    session: Session::new(),
    api_id: API_ID,
    api_hash: API_HASH.to_string(),
    params: Default::default(),
}).await?;

// Authenticate
client.request_login_code(phone, api_id, api_hash).await?;
client.sign_in(&token, code).await?;

// Send message (upload file)
client.send_message(chat, message).await?;

// Get messages (list files)
let messages = client.get_messages(chat).await?;
```

## Future Enhancements

1. **Progressive Upload**: Upload while selecting more files
2. **Deduplication**: Don't upload the same file twice
3. **Sharing**: Share files with other Telegram users
4. **Sync**: Real-time sync between devices
5. **Search**: Full-text search in file contents
6. **Versioning**: Keep multiple versions of files
7. **Trash**: Soft delete with recovery option

## Limitations

1. **File Size**: 2GB per chunk (Telegram limit)
2. **API Rate Limits**: Telegram throttles excessive requests
3. **Storage**: Technically unlimited, but abuse may result in ban
4. **Speed**: Limited by Telegram's CDN and your connection
5. **Reliability**: Depends on Telegram's uptime

## Development Tips

### Testing Authentication

Use a test phone number to avoid rate limits on your main account.

### Debugging Telegram API

Enable debug logging in grammers:

```rust
env_logger::Builder::from_default_env()
    .filter_level(log::LevelFilter::Debug)
    .init();
```

### Local Development

The app uses mock data in development mode for faster testing without hitting Telegram's API.

## Troubleshooting Common Issues

### "Session expired"

Delete `tvault_session.bin` and log in again.

### "Rate limit exceeded"

Wait a few minutes before making more requests.

### "File too large"

Files are automatically chunked, but very large files (>2GB per message) need special handling.

---

For more information, see the [README.md](README.md) and [SETUP_GUIDE.md](SETUP_GUIDE.md).
