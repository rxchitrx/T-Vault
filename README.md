# T-Vault Desktop App 🚀

> Unlimited cloud storage powered by Telegram - Built for macOS

![T-Vault](https://img.shields.io/badge/Platform-macOS-blue?style=flat-square)
![License](https://img.shields.io/badge/License-MIT-green?style=flat-square)
![Tauri](https://img.shields.io/badge/Built%20with-Tauri-orange?style=flat-square)

## ✨ Features

- **🔒 Secure Login**: Seamless authentication with your Telegram account
- **📂 File Management**: Upload, download, and organize files in folders
- **🖼️ Gallery View**: Beautiful gallery for photos and videos
- **💾 Unlimited Storage**: Leverage Telegram's infrastructure for unlimited file storage
- **🎨 Native macOS UI**: Beautiful, modern interface that feels right at home on macOS
- **🔐 In-App Setup**: Easy API key configuration through the UI

## 🚨 Important Notice

This application uses Telegram as a storage backend by uploading files to your Telegram "Saved Messages". Please note:

- This is a **gray area** usage of Telegram's platform
- Use responsibly and avoid excessive automated uploads
- Your files are stored on Telegram's servers
- Telegram could change their policies at any time
- **Not recommended as your only backup solution**

## 📋 Prerequisites

Before you begin, ensure you have the following installed:

- **Node.js** (v20 or higher)
- **Rust** (latest stable version)

## 🛠️ Quick Start

### 1. Clone the Repository

```bash
git clone https://github.com/inulute/t-vault.git
cd t-vault
```

### 2. Install Dependencies

```bash
npm install
```

### 3. Get Your Telegram API Credentials

1. Go to [https://my.telegram.org](https://my.telegram.org)
2. Log in with your phone number
3. Click "API development tools"
4. Create a new application
5. Copy your `api_id` and `api_hash`

### 4. Run the Application

```bash
npm run tauri:dev
```

### 5. First-Time Setup

When you first launch the app:

1. **Enter API Credentials**: The app will prompt you to enter your Telegram API ID and API Hash
   - The app validates your credentials before saving
   - Your credentials are stored securely on your device

2. **Login**: After saving your API keys, log in with your Telegram phone number
   - Enter your phone number with country code (e.g., `+1234567890`)
   - Enter the verification code sent to your Telegram app

3. **Start Using**: You're ready to upload and manage files!

## 🏗️ Building for Production

```bash
npm run tauri:build
```

The built app will be in `src-tauri/target/release/bundle/`.

## 🎯 How It Works

T-Vault uses a clever approach to provide unlimited storage:

1. **Authentication**: You log in with your Telegram account using their official API
2. **File Upload**: Files are uploaded to your Telegram "Saved Messages"
3. **Metadata Storage**: A JSON structure tracks your folder organization
4. **Retrieval**: Files are downloaded from Telegram when needed
5. **Sync**: Files are accessible from any device with Telegram

## 📱 Project Structure

```
t-vault/
├── src/                      # React frontend
│   ├── components/          # UI components
│   │   ├── ApiKeyScreen.tsx # API key entry screen
│   │   ├── LoginScreen.tsx  # Telegram login
│   │   ├── Dashboard.tsx    # Main dashboard
│   │   └── ...             # Other components
│   ├── App.tsx             # Main app component
│   ├── main.tsx            # Entry point
│   └── styles.css          # Global styles
├── src-tauri/               # Rust backend
│   ├── src/
│   │   ├── main.rs         # Tauri main
│   │   ├── telegram.rs     # Telegram API integration
│   │   ├── storage.rs      # File storage logic
│   │   ├── api_keys.rs     # API key management
│   │   └── encryption.rs   # Encryption utilities (unused)
│   ├── Cargo.toml          # Rust dependencies
│   └── tauri.conf.json     # Tauri configuration
├── scripts/                 # Build scripts
├── package.json            # Node dependencies
└── README.md               # This file
```

## 🔐 Security & Privacy

- **Secure Storage**: API keys are stored in the app's data directory (not in the project)
- **No Hardcoded Credentials**: All credentials are user-provided
- **Secure Sessions**: Telegram sessions are stored locally
- **No Third Parties**: Direct communication with Telegram's API
- **Your Data**: Everything stays in your Telegram account

## 🐛 Troubleshooting

### "Invalid API credentials" error

- Double-check your API ID and API Hash from [my.telegram.org](https://my.telegram.org)
- Make sure there are no extra spaces or characters
- Try getting new API credentials

### "Failed to connect to Telegram"

- Check your internet connection
- Verify your API credentials are correct
- Ensure you're not behind a restrictive firewall

### "Session expired"

- The app will prompt you to log in again
- Your files remain safe in Telegram

### Build errors

```bash
# Clean and rebuild
rm -rf node_modules src-tauri/target
yarn install
cargo clean
yarn tauri:dev
```

## 🗺️ Roadmap

- [x] macOS Desktop App
- [x] In-app API key configuration
- [x] API key validation
- [ ] iOS Companion App
- [ ] Real-time sync between devices
- [ ] File sharing with other users
- [ ] Advanced search capabilities
- [ ] Video/audio preview
- [ ] Batch operations

## 🤝 Contributing

Contributions are welcome! Please feel free to submit a Pull Request.

1. Fork the repository
2. Create your feature branch (`git checkout -b feature/AmazingFeature`)
3. Commit your changes (`git commit -m 'Add some AmazingFeature'`)
4. Push to the branch (`git push origin feature/AmazingFeature`)
5. Open a Pull Request

## 📄 License

This project is licensed under the MIT License - see the [LICENSE](LICENSE) file for details.

## ⚠️ Disclaimer

This application is not affiliated with or endorsed by Telegram. Use at your own risk. The developers are not responsible for any account restrictions or data loss that may occur from using this application.

## 💖 Acknowledgments

- Built with [Tauri](https://tauri.app/)
- Powered by [Telegram](https://telegram.org/)
- Inspired by the original [T-Vault](https://github.com/inulute/t-vault)

## 📞 Support

If you encounter any issues or have questions:

- Open an issue on GitHub
- Check existing issues for solutions
- Read the troubleshooting section above

---

Made with ❤️ for the macOS community
