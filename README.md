<h1 align="center">
    <picture>
            <img src="https://raw.githubusercontent.com/Phieu-Tran/ClipPaste/refs/heads/main/src-tauri/icons/64x64.png" alt="ClipPaste" width="48">
        </picture>
        <br>
        ClipPaste
</h1>

<p align="center">
    <strong>A beautiful clipboard history manager for Windows, macOS &amp; Linux</strong>
</p>

<p align="center">
    <a href="https://github.com/Phieu-Tran/ClipPaste/releases/latest"><img src="https://img.shields.io/github/v/release/Phieu-Tran/ClipPaste?style=for-the-badge&color=blue&label=Download" alt="Download"></a>
    <a href="https://github.com/Phieu-Tran/ClipPaste/releases"><img src="https://img.shields.io/github/downloads/Phieu-Tran/ClipPaste/total?style=for-the-badge&color=green&label=Downloads" alt="Total Downloads"></a>
    <a href="LICENSE"><img src="https://img.shields.io/badge/License-GPL%203.0-orange.svg?style=for-the-badge" alt="GPL-3.0 License"></a>
</p>

<p align="center">
    Built with <strong>Rust + Tauri + React + TypeScript</strong> — fast, private, and lightweight.
</p>

---

## Screenshots

<p align="center">
    <img src="docs/clippaste_dark.png" alt="Dark theme" width="100%">
</p>

<p align="center">
    <img src="docs/clippaste_light.png" alt="Light theme" width="100%">
</p>

---

## Features

| | Feature | Description |
|:---:|:---|:---|
| 🔒 | **Private** | All data stored locally, never leaves your machine |
| ⚡ | **Fast & Lightweight** | Built with Rust for native performance |
| 📌 | **Per-Folder Pin** | Pin clips to the top — scoped to each folder individually |
| ✏️ | **Edit Before Paste** | Modify text content before pasting |
| 📁 | **Folders** | Organize clips into color-coded folders with drag & drop |
| 👀 | **Hover Preview** | Hover a folder tab to preview its clips without switching |
| 🔍 | **Unified Search** | Search filters both clips and folder tabs simultaneously |
| 🎨 | **Themes & Effects** | Dark / Light / System with Mica, Mica Alt, and native rounded corners |
| 🖥️ | **Multi-Monitor** | Window appears on the active display |
| 🚫 | **Ignore Apps** | Exclude sensitive apps (password managers, etc.) |
| ⌨️ | **Custom Hotkey** | Set your preferred shortcut (default: `Ctrl+Shift+V`) |
| 🔄 | **Infinite Scroll** | Seamlessly browse unlimited history |
| 🛡️ | **Smart Filtering** | Ignore "Ghost Copies" from other clipboard tools |
| 🗂️ | **Folder Protection** | Folder items survive bulk clear operations |
| 📂 | **Custom Data Dir** | Choose where to store your clipboard database |
| 🔄 | **Auto-Update** | In-app update with download progress bar |

---

## Installation

### Download

> **[Download the latest release](https://github.com/Phieu-Tran/ClipPaste/releases/latest)**

| Platform | Architecture | Format |
|:---------|:-------------|:-------|
| **Windows** | x64, ARM64 | `.exe` (NSIS installer) |
| **macOS** | Apple Silicon (M1+), Intel | `.dmg` |
| **Linux** | x64 | `.deb`, `.AppImage` |

### Platform Notes

| Feature | Windows | macOS | Linux |
|:--------|:-------:|:-----:|:-----:|
| Clipboard monitoring | ✅ | ✅ | ✅ |
| Auto-paste | ✅ | ❌ | ❌ |
| Source app detection | ✅ | ❌ | ❌ |
| Source app icon | ✅ | ❌ | ❌ |
| Window effects (Mica) | ✅ | Vibrancy | ❌ |
| Auto-start | ✅ | ✅ | ✅ |
| Custom hotkey | ✅ | ✅ | ✅ |

> **macOS / Linux**: Core clipboard history works. Source app detection and auto-paste are Windows-only for now.

### Security (Windows)

Every release is scanned with [VirusTotal](https://www.virustotal.com/) (70+ antivirus engines). Some AI-based engines may flag the installer as a false positive because ClipPaste monitors the clipboard and sends keyboard input — behaviors shared with legitimate clipboard managers.

> If your antivirus blocks ClipPaste, add it to your exclusion list or [report a false positive](https://www.virustotal.com/).

---

## Keyboard Shortcuts

| Shortcut | Action |
|:---------|:-------|
| `Ctrl+Shift+V` | Toggle window *(customizable)* |
| `Ctrl+F` | Focus search bar |
| `Escape` | Close window / Clear search |
| `Enter` | Paste selected clip |
| `Ctrl+Delete` | Delete selected clip |
| `P` | Pin / Unpin selected clip |
| `E` | Edit before paste *(text only)* |
| `↑` `↓` | Navigate between clips |

---

## Tech Stack

| Layer | Technology |
|:------|:-----------|
| Framework | [Tauri v2](https://tauri.app/) |
| Frontend | React 18 + TypeScript + Vite |
| Styling | TailwindCSS v3 |
| Backend | Rust (Tokio async runtime) |
| Database | SQLite via sqlx |
| Window Effects | [window-vibrancy](https://github.com/Phieu-Tran/window-vibrancy) *(custom fork)* |

---

## Building from Source

### Prerequisites

- [Node.js](https://nodejs.org/) 18+
- [Rust](https://rustup.rs/) 1.70+
- [pnpm](https://pnpm.io/)

**Linux additional dependencies:**
```bash
sudo apt install libwebkit2gtk-4.1-dev libgtk-3-dev libayatana-appindicator3-dev librsvg2-dev patchelf
```

```bash
# Install dependencies
pnpm install

# Development
pnpm tauri dev

# Production build
pnpm tauri build
```

---

## Application Exceptions

ClipPaste can exclude specific apps from clipboard history — useful for password managers and banking apps.

- **Settings → Ignored Applications** — browse for an executable or type its name
- On Windows: matches by **executable name** (`notepad.exe`) or **full path** (`C:\Windows\System32\notepad.exe`)
- Case-insensitive matching

---

## Architecture

```
ClipPaste/
├── src-tauri/              # Rust backend
│   ├── src/
│   │   ├── lib.rs          # Core logic, window animation, tray
│   │   ├── clipboard.rs    # Clipboard monitoring & processing
│   │   ├── database.rs     # SQLite pool + migrations
│   │   ├── commands.rs     # All Tauri IPC commands
│   │   └── models.rs       # Data models
│   └── tauri.conf.json
├── frontend/               # React frontend
│   ├── src/
│   │   ├── App.tsx         # Root component
│   │   ├── components/     # ClipList, ClipCard, ControlBar...
│   │   └── hooks/          # useKeyboard, useTheme
└── README.md
```

### Design Decisions

- **Hybrid Clipboard**: Frontend writes images via `navigator.clipboard.write` (stable on WebView2), backend handles text + database + paste trigger
- **Shift+Insert** for pasting: works in terminals (PowerShell, WSL) where `Ctrl+V` sends a control character
- **Flicker-free effects**: Uses [window-vibrancy](https://github.com/Phieu-Tran/window-vibrancy) `switch_effect()` to clear old + apply new DWM effect in one call
- **Native rounded corners**: DWM `DWMWA_WINDOW_CORNER_PREFERENCE` on Windows 11

---

## License

[GPL-3.0](LICENSE)
