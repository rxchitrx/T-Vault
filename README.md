# T-Vault Desktop App 🚀

> Unlimited cloud storage powered by Telegram - Built for macOS

<video src="https://github.com/rxchitrx/T-Vault/raw/main/Media/T-vault.mp4" width="100%" controls></video>

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

## 🚀 Getting Started

### 1. Download
Download the latest `.dmg` file from the [Releases](../../releases) page.

### 2. Install
Open the `.dmg` file and drag **T-Vault** to your **Applications** folder.

### 3. Setup API Credentials
To use T-Vault, you need your own Telegram API keys:

1. Go to [my.telegram.org](https://my.telegram.org) and log in.
2. Click on **API development tools**.
3. Create a new application (you can use any name).
4. Copy your `App api_id` and `App api_hash`.
5. Open T-Vault and enter these credentials in the setup screen.

![T-Vault Setup](Media/T-vault.png)

### 4. Login
After configuring your API keys, log in with your Telegram phone number and enter the verification code sent to your Telegram app.

## 📸 Gallery

Experience your media like never before with our native gallery view. T-Vault automatically categorizes your photos and videos, providing a seamless browsing experience directly from your Telegram storage.

*(Check out the video at the top for a full demo of the gallery in action!)*

## 🙏 Requests

Have a feature in mind or found a bug? We'd love to hear from you!
- **Feature Requests**: Open an issue with the `enhancement` label.
- **Bug Reports**: Open an issue with the `bug` label.
- **General Feedback**: Join our GitHub Discussions.

## 🎯 How It Works

T-Vault uses a clever approach to provide unlimited storage:

1. **Authentication**: You log in with your Telegram account using their official API.
2. **File Upload**: Files are uploaded to your Telegram "Saved Messages".
3. **Metadata Storage**: A JSON structure tracks your folder organization.
4. **Retrieval**: Files are downloaded from Telegram when needed.
5. **Sync**: Files are accessible from any device with Telegram.

## 🔐 Security & Privacy

- **Secure Storage**: API keys are stored in the app's data directory (not in the project).
- **No Hardcoded Credentials**: All credentials are user-provided.
- **Secure Sessions**: Telegram sessions are stored locally.
- **No Third Parties**: Direct communication with Telegram's API.
- **Your Data**: Everything stays in your Telegram account.

## 🐛 Troubleshooting

### "Invalid API credentials" error
- Double-check your API ID and API Hash from [my.telegram.org](https://my.telegram.org).
- Make sure there are no extra spaces or characters.

### "Failed to connect to Telegram"
- Check your internet connection.
- Ensure you're not behind a restrictive firewall.

### "Session expired"
- The app will prompt you to log in again. Your files remain safe in Telegram.

## 🛠️ Development

If you'd like to build T-Vault from source or contribute to the project:

### Prerequisites
- **Node.js** (v20 or higher)
- **Rust** (latest stable version)

### Build Instructions
```bash
# Clone the repository
git clone https://github.com/inulute/t-vault.git
cd t-vault

# Install dependencies
npm install

# Run in development mode
npm run tauri:dev

# Build for production
npm run tauri:build
```

### Project Structure
```
t-vault/
├── src/                      # React frontend
├── src-tauri/               # Rust backend
│   ├── src/
│   │   ├── telegram.rs     # Telegram API integration
│   │   └── storage.rs      # File storage logic
└── ...
```

## 🗺️ Roadmap

- [x] macOS Desktop App
- [x] In-app API key configuration
- [ ] iOS Companion App
- [ ] Real-time sync between devices
- [ ] File sharing with other users
- [ ] Advanced search capabilities

*If we receive enough interest and requests, we plan to expand T-Vault with official iOS and Windows applications!*

## 🤝 Contributing

Contributions are welcome! Please feel free to submit a Pull Request.

## 📄 License

This project is licensed under the MIT License - see the [LICENSE](LICENSE) file for details.

## ⚠️ Disclaimer

This application is not affiliated with or endorsed by Telegram. Use at your own risk.

## 💖 Acknowledgments

- Built with [Tauri](https://tauri.app/)
- Powered by [Telegram](https://telegram.org/)
- Inspired by [Unlim](https://github.com/inulute/unlim-cloud)

## 📞 Support

If you encounter any issues, please open an issue on GitHub.

---

Made with ❤️ for the macOS community
