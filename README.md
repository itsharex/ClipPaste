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
    Built with <strong>Rust + Tauri v2 + React 18 + TypeScript</strong> — fast, private, and lightweight.
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

### Clipboard

| | Feature | Description |
|:---:|:---|:---|
| 🔒 | **Private & Local** | All data stored locally — never leaves your machine |
| ⚡ | **Fast & Lightweight** | Rust backend, ~50MB RAM, instant search |
| 🔍 | **Smart Search** | Multi-word AND search with relevance ranking (exact match first) |
| 🏷️ | **Content Detection** | Auto-detect URLs, emails, color codes, file paths — styled cards |
| 📌 | **Per-Folder Pin** | Pin clips to the top within each folder |
| ✏️ | **Edit Before Paste** | Modify text content before pasting |
| 📋 | **Paste as Plain Text** | Strip formatting and paste clean text |
| 📝 | **Notes** | Add annotations to any clip |
| 🖼️ | **Image on Disk** | Images stored as files, not in DB — keeps database small |

### Organization

| | Feature | Description |
|:---:|:---|:---|
| 📁 | **Folders** | Color-coded folders with drag & drop |
| 👀 | **Hover Preview** | Preview folder contents without switching |
| 🗂️ | **Folder Protection** | Folder items survive bulk clear operations |
| 🔢 | **Paste Count** | Track how many times each clip is pasted |

### Dashboard & History

| | Feature | Description |
|:---:|:---|:---|
| 📊 | **Dashboard** | Stats overview — total clips, today, images, folders |
| 📅 | **History Timeline** | Browse clips by date with calendar picker |
| 📈 | **Activity Chart** | Clips per day (last 7 days), clickable bars |
| 🏆 | **Top Apps** | Most used source apps with visual bar chart |
| 💾 | **Export / Import** | Backup & restore as zip (DB + images) |

### Appearance & System

| | Feature | Description |
|:---:|:---|:---|
| 🎨 | **Themes & Effects** | Dark / Light / System + Mica, Mica Alt effects |
| 🖥️ | **Multi-Monitor** | Window appears on the active display |
| 🚫 | **Ignore Apps** | Exclude password managers, banking apps, etc. |
| ⌨️ | **Custom Hotkey** | Default: `Ctrl+Shift+V` |
| 🔄 | **Auto-Update** | In-app update with progress bar |
| 📂 | **Custom Data Dir** | Choose where to store your data |

---

## Installation

### Download

> **[Download the latest release](https://github.com/Phieu-Tran/ClipPaste/releases/latest)**

| Platform | Architecture | Format |
|:---------|:-------------|:-------|
| **Windows** | x64, ARM64 | `.exe` (NSIS), `.msi` |
| **macOS** | Apple Silicon (M1+), Intel | `.dmg` |
| **Linux** | x64 | `.deb`, `.AppImage`, `.rpm` |

### Platform Support

| Feature | Windows | macOS | Linux |
|:--------|:-------:|:-----:|:-----:|
| Clipboard monitoring | ✅ | ✅ | ✅ |
| Auto-paste | ✅ (Shift+Insert) | ✅ (Cmd+V) | ❌ |
| Source app detection | ✅ | ✅ | ❌ |
| Source app icon | ✅ | ❌ | ❌ |
| Window effects | Mica / Mica Alt | Vibrancy | ❌ |
| Drag-copy to apps | ✅ | ✅ | ✅ |

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

## Architecture

```
┌─────────────────────────────────────────────────────────────┐
│                        ClipPaste                            │
├──────────────────────────┬──────────────────────────────────┤
│     Frontend (React)     │       Backend (Rust/Tauri)       │
│                          │                                  │
│  ┌──────────────────┐    │    ┌─────────────────────────┐   │
│  │    ControlBar     │    │    │     clipboard.rs         │   │
│  │  Search + Folders │    │    │  Monitor → Debounce →   │   │
│  └────────┬─────────┘    │    │  Detect Subtype → Save   │   │
│           │              │    └──────────┬──────────────┘   │
│  ┌────────▼─────────┐    │               │                  │
│  │     ClipList      │    │    ┌──────────▼──────────────┐   │
│  │  @tanstack/virtual│◄───┼────┤     commands.rs          │   │
│  │  (horizontal)     │ IPC│    │  get_clips, search,     │   │
│  └────────┬─────────┘    │    │  paste, delete, export   │   │
│           │              │    └──────────┬──────────────┘   │
│  ┌────────▼─────────┐    │               │                  │
│  │     ClipCard      │    │    ┌──────────▼──────────────┐   │
│  │  Subtype-aware    │    │    │     database.rs          │   │
│  │  URL/Email/Color  │    │    │  SQLite (DELETE mode)    │   │
│  └──────────────────┘    │    │  + Migration versioning  │   │
│                          │    └──────────┬──────────────┘   │
│  ┌──────────────────┐    │               │                  │
│  │   SettingsPanel   │    │    ┌──────────▼──────────────┐   │
│  │  Dashboard + Stats│    │    │     Storage              │   │
│  │  History Timeline │    │    │  clipboard.db (text)     │   │
│  └──────────────────┘    │    │  images/*.png (on disk)   │   │
│                          │    └─────────────────────────┘   │
└──────────────────────────┴──────────────────────────────────┘
```

### Data Flow

```
User copies text/image
        │
        ▼
┌─────────────────┐     ┌──────────────────┐     ┌─────────────┐
│ Clipboard Plugin │────▶│  clipboard.rs     │────▶│  SQLite DB   │
│ (OS clipboard)   │     │  - debounce 150ms │     │  - text in DB│
│                  │     │  - detect subtype │     │  - img on    │
│                  │     │  - SHA256 dedup   │     │    disk      │
└─────────────────┘     │  - source app info│     └──────┬──────┘
                        └──────────┬────────┘            │
                                   │                     │
                                   ▼                     ▼
                        ┌──────────────────┐     ┌─────────────┐
                        │ emit event       │     │ Search cache │
                        │ → frontend reload│     │ (in-memory)  │
                        └──────────────────┘     └─────────────┘
```

### Storage Layout

```
{data_dir}/ClipPaste/
├── clipboard.db           # SQLite (DELETE journal mode)
└── images/                # Clipboard images
    ├── {sha256}.png
    └── ...
```

### Key Design Decisions

| Decision | Reason |
|:---------|:-------|
| **SQLite DELETE mode** (not WAL) | Clipboard manager writes rarely — data safety > write speed |
| **Images on disk** | DB stays small (~2MB), images in separate files |
| **In-memory search cache** | Instant single-word search (<1ms for 1000+ clips) |
| **Multi-word AND search** | "docker compose" matches clips containing both words |
| **Relevance sorting** | Exact substring matches rank above partial word matches |
| **Shift+Insert** for paste | Works in terminals (PowerShell, WSL) where Ctrl+V doesn't |
| **@tanstack/react-virtual** | Horizontal virtual list — constant DOM count regardless of clip count |
| **Hard delete** (no soft delete) | No DB bloat, no stale rows, simpler queries |
| **Schema version tracking** | Migrations run once per version, skip if already applied |

---

## Tech Stack

| Layer | Technology |
|:------|:-----------|
| Framework | [Tauri v2](https://tauri.app/) |
| Frontend | React 18 + TypeScript + Vite |
| Styling | TailwindCSS v3 + tailwind-merge |
| Virtual List | [@tanstack/react-virtual](https://tanstack.com/virtual) |
| Backend | Rust (Tokio async runtime) |
| Database | SQLite via [sqlx](https://github.com/launchbadge/sqlx) |
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

# Run tests
cd tests && npx vitest run
```

---

## License

[GPL-3.0](LICENSE)
