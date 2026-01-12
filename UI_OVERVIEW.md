# 🎨 UnlimCloud UI Overview

This document shows what each screen looks like and what you can do.

## 🔐 Login Screen

**What you see:**
- UnlimCloud logo (cloud icon)
- Phone number input field
- "Send Code" button
- Clean, modern design with Telegram blue accent

**What you do:**
1. Enter phone number with country code (+1234567890)
2. Click "Send Code"
3. Enter the verification code from Telegram
4. Click "Verify Code"

**Design features:**
- Gradient background (Telegram blue)
- Glass-effect white card
- Error messages with icons
- Smooth transitions

---

## 📁 File Manager (Main Screen)

**Layout:**
```
┌─────────────────────────────────────────────────────────┐
│ 🔵 UnlimCloud        │  My Files    💾 Storage Stats    │ ← Titlebar
├──────────────┬──────────────────────────────────────────┤
│              │  Home > Documents          [Upload] [New Folder] │
│  📂 Files    │  ┌─────────────────────────────────────┐ │
│  🖼️ Gallery  │  │ 🔍 Search files...                   │ │
│  ⚙️ Settings │  └─────────────────────────────────────┘ │
│              │                                           │
│              │  📁 Documents                    [⋮]      │
│              │  📁 Photos                       [⋮]      │
│              │  📄 Report.pdf (2.5 MB)         [↓][🗑️]  │
│              │  🖼️ Vacation.jpg (1.2 MB)      [↓][🗑️]  │
│              │                                           │
│  Powered by  │                                           │
│  Telegram    │                                           │
│  Unlimited   │                                           │
│  Storage     │                                           │
└──────────────┴──────────────────────────────────────────┘
```

**Features:**
- **Breadcrumb navigation** (Home > Documents > ...)
- **Search bar** to filter files
- **Upload button** to add files
- **New Folder button** to create folders
- **File list** with icons, names, sizes
- **Hover actions** (download, delete)
- **Sidebar navigation**

---

## 🖼️ Gallery View

**Layout:**
```
┌─────────────────────────────────────────────────────────┐
│ 🔵 UnlimCloud        │  Gallery    💾 Storage Stats     │
├──────────────┬──────────────────────────────────────────┤
│              │  Gallery (24 items)      [Grid] [List]   │
│  📂 Files    │                                           │
│  🖼️ Gallery  │  ┌───────┐ ┌───────┐ ┌───────┐ ┌───────┐│
│  ⚙️ Settings │  │ 🖼️    │ │ 🖼️    │ │ 🎬    │ │ 🖼️    ││
│              │  │Photo1 │ │Photo2 │ │Video1 │ │Photo3 ││
│              │  └───────┘ └───────┘ └───────┘ └───────┘│
│              │  ┌───────┐ ┌───────┐ ┌───────┐ ┌───────┐│
│              │  │ 🖼️    │ │ 🖼️    │ │ 🖼️    │ │ 🎬    ││
│              │  │Photo4 │ │Photo5 │ │Photo6 │ │Video2 ││
│              │  └───────┘ └───────┘ └───────┘ └───────┘│
│              │                                           │
└──────────────┴──────────────────────────────────────────┘
```

**Features:**
- **Grid view** (default) - thumbnails in grid
- **List view** - detailed list with file info
- **View toggle** buttons
- **Click to preview** full size
- **Hover actions** (download, delete)
- **Filters** images and videos only

---

## ⚙️ Settings Screen

**Sections:**

### 📱 Account
- Connected account (Telegram)
- Sign out button
- Storage backend info

### 🔐 Security
- **End-to-End Encryption** toggle
  - Encrypt files before uploading
- **Change Encryption Password** button

### 🔔 Preferences
- **Notifications** toggle
  - Upload/download progress alerts
- **Auto Sync** toggle
  - Automatic sync with Telegram

### ℹ️ About
- Version number
- Description
- Links to GitHub and Telegram
- Important disclaimer

---

## 🎨 Design System

### Colors
- **Primary**: Telegram Blue (#0088cc)
- **Background**: Light Gray (#F9FAFB)
- **Cards**: White with shadow
- **Text**: Dark Gray (#111827)

### Components
- **Buttons**: Rounded, hover effects, active states
- **Cards**: Rounded corners, subtle shadows
- **Inputs**: Bordered, focus states with blue ring
- **Icons**: Lucide React icon set

### Typography
- **Font**: SF Pro (macOS system font)
- **Headings**: Bold, large
- **Body**: Regular, readable

### Interactions
- **Hover effects** on all interactive elements
- **Smooth transitions** (200ms)
- **Active states** (scale down slightly)
- **Loading states** with spinners

---

## 🖱️ User Interactions

### File Operations
- **Upload**: Click Upload → Select files → Progress indicator → Success
- **Download**: Hover file → Click download icon → Save dialog
- **Delete**: Hover file → Click delete icon → Confirm → Removed
- **Create Folder**: Click New Folder → Enter name → Created

### Navigation
- **Sidebar**: Click sections to switch views
- **Breadcrumbs**: Click to navigate up folder tree
- **Search**: Type to filter files instantly

### Feedback
- **Success**: Green messages
- **Errors**: Red messages with icons
- **Loading**: Spinners and skeleton screens
- **Empty states**: Helpful illustrations

---

## 📱 Responsive Behavior

The app adjusts to different window sizes:

- **Minimum**: 800x600 (enforced in config)
- **Optimal**: 1200x800
- **Maximum**: Full screen

UI elements adapt:
- Sidebar stays fixed width (256px)
- Content area grows/shrinks
- Grid adjusts column count
- Text truncates with ellipsis

---

## ✨ Special Features

### macOS Integration
- **Transparent titlebar** (native look)
- **Drag region** (move window)
- **System font** (SF Pro)
- **Native dialogs** (file picker, alerts)

### Animations
- **Fade in** on load
- **Slide in** for sidebar
- **Bounce** for loading dots
- **Scale** on button press

### Accessibility
- **Keyboard navigation** (tab through elements)
- **Focus indicators** (blue outlines)
- **Alt text** for icons
- **High contrast** colors

---

## 🎯 Next Steps

1. **Run the app** to see it in action
2. **Upload some files** to test functionality
3. **Explore the gallery** with photos/videos
4. **Customize settings** to your preference

Enjoy your beautiful, unlimited cloud storage! 🚀
