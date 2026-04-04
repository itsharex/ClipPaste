<h1 align="center">
    <picture>
            <img src="https://raw.githubusercontent.com/Phieu-Tran/ClipPaste/refs/heads/main/src-tauri/icons/64x64.png" alt="ClipPaste" width="48">
        </picture>
        <br>
        ClipPaste
</h1>

<p align="center">
    <strong>A beautiful clipboard history manager for Windows &amp; Linux</strong>
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
| 🔍 | **Smart Search** | Multi-word AND search + fuzzy matching, relevance-ranked (exact phrase first) |
| 🏷️ | **Content Detection** | Auto-detect URLs, emails, color codes, file paths — styled cards |
| 🛡️ | **Sensitive Detection** | Auto-detect API keys, tokens, credit cards — blurred + shield icon |
| 👁️ | **Incognito Mode** | Pause clipboard recording with one click |
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
| 🔥 | **Frequently Pasted** | Smart folder showing clips pasted 5+ times |

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
| 🎨 | **Themes & Effects** | Dark / Light / System + Mica, Mica Alt, Acrylic, Blur effects |
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
| **Linux** | x64 | `.deb`, `.AppImage`, `.rpm` |

### Platform Support

| Feature | Windows | Linux |
|:--------|:-------:|:-----:|
| Clipboard monitoring | ✅ | ✅ |
| Auto-paste | ✅ (Shift+Insert) | ❌ |
| Source app detection | ✅ | ❌ |
| Source app icon | ✅ | ❌ |
| Window effects | Mica / Mica Alt / Acrylic / Blur | ❌ |
| Drag-copy to apps | ✅ | ✅ |

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

### System Overview

```mermaid
graph TB
    subgraph Frontend["Frontend (React 18 + TypeScript)"]
        App["App.tsx<br/><i>Orchestrator</i>"]

        subgraph Hooks["Custom Hooks"]
            useClipActions["useClipActions<br/><small>load, paste, copy, delete, pin, note</small>"]
            useFolderActions["useFolderActions<br/><small>CRUD, reorder, move clip</small>"]
            useDragDrop["useDragDrop<br/><small>card → folder drag</small>"]
            useFolderPreview["useFolderPreview<br/><small>hover preview + cache</small>"]
            useKeyboard["useKeyboard<br/><small>Esc, Ctrl+F, arrows, Enter</small>"]
            useBatchActions["useBatchActions<br/><small>bulk delete, move, paste</small>"]
            useContextMenu["useContextMenu<br/><small>right-click menu state</small>"]
            useFolderModal["useFolderModal<br/><small>create/rename modal</small>"]
        end

        subgraph Components["UI Components"]
            ControlBar["ControlBar<br/><small>search, folder tabs, filters</small>"]
            ClipList["ClipList<br/><small>@tanstack/react-virtual</small>"]
            ClipCard["ClipCard<br/><small>subtype badges, highlight, timestamp</small>"]
            Settings["SettingsPanel<br/><small>GeneralTab / FoldersTab / DashboardTab</small>"]
        end

        App --> Hooks
        App --> Components
    end

    subgraph Backend["Backend (Rust + Tauri v2)"]
        subgraph Commands["commands/"]
            clips["clips.rs<br/><small>get, paste, copy, delete, search, pin</small>"]
            folders["folders.rs<br/><small>CRUD, reorder</small>"]
            settings["settings.rs<br/><small>get/save, ignored apps, hotkey</small>"]
            data["data.rs<br/><small>export, import, dashboard, timeline</small>"]
            window["window.rs<br/><small>show, hide, focus, dragging</small>"]
        end

        clipboard["clipboard.rs<br/><small>monitor, debounce, dedup, subtype detect,<br/>sensitive detect, incognito mode</small>"]
        database["database.rs<br/><small>SQLite pool, migrations v1-v5</small>"]
        caches["In-Memory Caches<br/><small>SEARCH_CACHE (HashMap 50K cap)<br/>SETTINGS_CACHE · ICON_CACHE (LRU 100)</small>"]
    end

    subgraph Storage["Storage (local disk)"]
        db[("clipboard.db<br/><small>SQLite WAL</small>")]
        images["images/<br/><small>{sha256}.png</small>"]
    end

    Components -- "invoke()" --> Commands
    Commands --> database
    clipboard --> database
    clipboard --> caches
    database --> db
    database --> images

    style Frontend fill:#1e293b,stroke:#3b82f6,color:#e2e8f0
    style Backend fill:#1e293b,stroke:#f59e0b,color:#e2e8f0
    style Storage fill:#1e293b,stroke:#10b981,color:#e2e8f0
    style Hooks fill:#0f172a,stroke:#6366f1,color:#c7d2fe
    style Components fill:#0f172a,stroke:#6366f1,color:#c7d2fe
    style Commands fill:#0f172a,stroke:#d97706,color:#fde68a
```

### Clipboard Data Flow

```mermaid
sequenceDiagram
    participant OS as OS Clipboard
    participant Plugin as tauri-plugin-clipboard-x
    participant Monitor as clipboard.rs
    participant Cache as SEARCH_CACHE
    participant DB as SQLite
    participant Disk as images/*.png
    participant UI as React Frontend

    OS->>Plugin: clipboard_changed event
    Plugin->>Monitor: event listener fires

    alt Incognito mode ON
        Monitor-->>Monitor: Skip (return immediately)
    end

    Note over Monitor: Capture source app<br/>BEFORE debounce
    Monitor->>Monitor: Debounce 150ms
    Monitor->>Monitor: SHA256 hash

    alt Duplicate (hash exists)
        Monitor->>DB: UPDATE created_at (bump to top)
    else New clip
        Monitor->>Monitor: Detect subtype (url/email/color/path)
        Monitor->>Monitor: Detect sensitive (API keys/CC/JWT)
        alt Image
            Monitor->>Disk: Save {hash}.png
            Monitor->>DB: INSERT (filename + is_sensitive)
        else Text
            Monitor->>DB: INSERT (text + is_sensitive)
        end
        Monitor->>Cache: add_to_search_cache()
    end

    Monitor->>UI: emit("clipboard-change")
    UI->>UI: Reload clip list + toast
```

### Paste Flow

```mermaid
sequenceDiagram
    participant User
    participant UI as React Frontend
    participant Cmd as commands/clips.rs
    participant OS as OS Clipboard
    participant Target as Target App

    User->>UI: Double-click / Enter / Ctrl+1..9
    UI->>Cmd: invoke("paste_clip", id)
    Cmd->>Cmd: Stop clipboard listener
    Cmd->>OS: Write content to clipboard
    Cmd->>Cmd: Set IGNORE_HASH (prevent self-capture)
    Cmd->>Cmd: Restart listener
    Cmd->>UI: animate_window_hide()
    UI-->>Target: Shift+Insert (Win) / Cmd+V (Mac)
    Note over UI,Target: 200ms delay before keystroke

    Note over User,UI: Copy (no paste)
    User->>UI: Click copy button
    UI->>Cmd: invoke("copy_clip", id)
    Cmd->>OS: Write to clipboard only
    Note over UI: Window stays open
```

### Storage Layout

```
{data_dir}/ClipPaste/
├── clipboard.db           # SQLite (WAL mode)
├── clipboard.db-wal       # Write-Ahead Log (concurrent reads + writes)
├── clipboard.db-shm       # Shared memory index for WAL
└── images/                # Clipboard images (not in DB)
    ├── {sha256}.png       # Deduplicated by content hash
    └── ...
```

### Key Design Decisions

| Decision | Reason |
|:---------|:-------|
| **SQLite WAL mode + tuned PRAGMAs** | Concurrent reads/writes, 8MB cache, 64MB mmap, 5s busy_timeout |
| **Images on disk** | DB stays small (~2MB), images in separate files |
| **In-memory search cache** | HashMap capped at 50K entries, instant multi-word + fuzzy search (<1ms) |
| **Relevance sorting** | Exact phrase > all words > note match > fuzzy; folder as tiebreaker |
| **Sensitive content detection** | Regex-based (no AI), auto-blur on card, is_sensitive column in DB |
| **Asset protocol for images** | `asset://` URL instead of base64 — 75% less IPC payload |
| **Shift+Insert** for paste | Works in terminals (PowerShell, WSL) where Ctrl+V doesn't |
| **@tanstack/react-virtual** | Horizontal virtual list — constant DOM count regardless of clip count |
| **Hard delete** (no soft delete) | No DB bloat, no stale rows, simpler queries |
| **Pinned + folder items protected** | Bulk clear, dedup, and auto-trim never touch pinned or folder clips |
| **Atomic DB operations** | Transactions for folder delete, max_items trim — crash-safe |
| **ICON_CACHE LRU(100)** | Bounded app icon cache prevents memory leak on long sessions |
| **Modular commands/** | 7 domain files instead of monolithic commands.rs (1500+ lines) |

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

# Run Rust tests
cd src-tauri && cargo test
```

---

## License

[GPL-3.0](LICENSE)
