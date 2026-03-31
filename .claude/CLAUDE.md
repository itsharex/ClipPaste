# ClipPaste – CLAUDE.md

## Project Overview

**ClipPaste** is a cross-platform clipboard history manager for **Windows, macOS, and Linux**, built with **Tauri v2** (Rust backend) + **React/TypeScript** (frontend). Package name: `clippaste`, version: `1.4.4`.

### Platform Support

| Feature | Windows | macOS | Linux |
|---------|---------|-------|-------|
| Clipboard monitoring | ✅ | ✅ | ✅ |
| Auto-paste (Shift+Insert) | ✅ | ❌ | ❌ |
| Source app detection | ✅ (Win32 API) | ❌ | ❌ |
| Source app icon | ✅ (Win32 API) | ❌ | ❌ |
| Window effects | Mica/Mica Alt/Clear | Vibrancy | ❌ |
| File/folder picker | PowerShell | osascript | zenity |

Windows-specific code (`windows` crate, Win32 API) is gated behind `#[cfg(target_os = "windows")]`. Non-Windows platforms get graceful fallbacks (no source app info, no auto-paste).

## Tech Stack

| Layer | Technology |
|-------|-----------|
| Framework | Tauri v2 |
| Frontend | React 18 + TypeScript + Vite |
| Styling | TailwindCSS v3 + tailwind-merge |
| Backend | Rust (Tokio async runtime) |
| Database | SQLite via sqlx 0.7 |
| Clipboard | tauri-plugin-clipboard-x |
| Window effects | window-vibrancy (custom fork by Phieu-Tran) |
| Global shortcut | tauri-plugin-global-shortcut |
| Auto-start | tauri-plugin-autostart |
| Analytics | tauri-plugin-aptabase |

## Directory Structure

```
ClipPaste/
├── frontend/src/          # React frontend
│   ├── App.tsx            # Root component, manages all state
│   ├── components/        # UI components
│   │   ├── ClipList.tsx   # Clipboard item list (virtual scroll)
│   │   ├── ClipCard.tsx   # Card for each clip
│   │   ├── ControlBar.tsx # Control bar (search, folders, settings)
│   │   ├── ContextMenu.tsx
│   │   ├── FolderModal.tsx
│   │   ├── SettingsPanel.tsx
│   │   └── DragPreview.tsx
│   ├── hooks/
│   │   ├── useKeyboard.ts # Keyboard shortcuts
│   │   └── useTheme.ts    # Theme management
│   ├── windows/
│   │   └── SettingsWindow.tsx
│   ├── types/index.ts     # TypeScript types
│   └── constants.ts       # Layout constants (WINDOW_HEIGHT=298, sync with Rust)
│
├── src-tauri/src/         # Rust backend
│   ├── main.rs            # Entry point (calls run_app())
│   ├── lib.rs             # run_app(), window animation, tray, hotkey setup
│   ├── commands.rs        # All Tauri commands (invoke handlers)
│   ├── clipboard.rs       # Clipboard monitoring & processing
│   ├── database.rs        # SQLite pool + migrations
│   ├── models.rs          # Rust structs (Clip, Folder, ClipboardItem, etc.)
│   └── constants.rs       # WINDOW_HEIGHT=330.0, WINDOW_MARGIN=0.0
│
└── src-tauri/
    ├── Cargo.toml         # Rust dependencies
    ├── tauri.conf.json    # Tauri config
    └── capabilities/default.json
```

## Database Schema (SQLite)

```sql
-- clipboard.db (location: %APPDATA%/ClipPaste/ or custom path)
clips (id, uuid, clip_type, content BLOB, text_preview, content_hash,
       folder_id, is_deleted, source_app, source_icon, metadata,
       created_at, last_accessed)
folders (id, name, icon, color, is_system, created_at)
settings (key TEXT PK, value TEXT)
ignored_apps (id, app_name UNIQUE)
```

## Tauri Commands (invoked from frontend)

```
get_clips, get_clip, paste_clip, delete_clip, search_clips
get_folders, create_folder, rename_folder, delete_folder, move_to_folder
get_settings, save_settings
get_clipboard_history_size, clear_clipboard_history, clear_all_clips, remove_duplicate_clips
register_global_shortcut, show_window, hide_window, focus_window
add_ignored_app, remove_ignored_app, get_ignored_apps
pick_file, pick_folder, get_layout_config
get_data_directory, set_data_directory
```

## Core Flows

1. **Clipboard monitoring**: `clipboard.rs::init()` → listens to `plugin:clipboard-x://clipboard_changed` → debounce 150ms → `process_clipboard_change()` → saves to DB → emits `clipboard-change` event
2. **Paste clip**: Frontend invokes `paste_clip` → backend stops listener → writes clipboard → animates window hide → callback: `send_paste_input()` (Shift+Insert)
3. **Window show/hide**: Slide animation from bottom of screen, 15 steps × 10ms. Monitor detected by cursor position (Windows Win32 API)
4. **Window effects (Windows)**: Mica / Mica Alt (Tabbed) / Clear, using `window-vibrancy` fork

## Settings

| Key | Default | Description |
|-----|---------|-------------|
| `hotkey` | `Ctrl+Shift+V` | Global shortcut to open the app |
| `theme` | `dark` | `light` / `dark` / `system` |
| `mica_effect` | `clear` | `clear` / `mica` / `mica_alt` |
| `auto_paste` | `true` | Auto-paste after selecting a clip |
| `max_items` | `1000` | Max number of clips to store |
| `ignore_ghost_clips` | `false` | Ignore clips with unknown source app |

## Data & Config Paths (Windows)

- **DB**: `%APPDATA%/ClipPaste/clipboard.db` (or custom path from config)
- **Config**: `%APPDATA%/ClipPaste/config.json` (stores `data_directory` if customized)
- **Logs**: App log directory (release mode only)
- **Migration**: Auto-migrates from old path `ClipPaste/paste_paw.db` → `clipboard.db`

## Build & Dev Commands

```bash
pnpm tauri dev          # Dev mode
pnpm tauri build        # Production build
pnpm build              # Frontend build only
pnpm format             # Prettier format frontend/src/**
```

## Important Notes

- `WINDOW_HEIGHT` must stay in sync between `constants.rs` (330.0) and `constants.ts` (298) — the values differ because one is physical pixels (backend) and the other is logical pixels (frontend)
- `auto_paste` uses Shift+Insert (not Ctrl+V) to avoid conflicts
- Settings window (`label: 'settings'`) is a separate WebviewWindow, URL: `index.html?window=settings`
- Clipboard dedup: uses SHA256 hash of content; if hash exists, bumps `created_at` (re-copy moves to top) and restores if soft-deleted
- Paste a clip updates `last_pasted_at` only — does NOT bump `created_at`, so pasting never reorders the list
- List sort order: `created_at DESC` (newest copy first, stable — paste does not change order)
- `ClipCard` has `data-clip-id={clip.id}` for DOM lookup; `ClipList` auto-scrolls selected card into view on `selectedClipId` change
- `CLIPBOARD_SYNC` mutex: prevents conflicts between clipboard monitor and paste operations
- `IS_ANIMATING` atomic flag: prevents race conditions during simultaneous show/hide
- Main window auto-hides on blur, unless the settings window is open
- Tray icon: `src-tauri/icons/tray.png`
- `bundle.publisher` in `tauri.conf.json` is set to `"Phieu-Tran"` — this controls the **Company** field shown in Windows Add/Remove Programs. Without it, Tauri extracts the middle segment of `identifier` (`me.xueshi.clipboard` → `xueshi`) as the publisher name

## Folder Protection Rules

Items saved in user-created folders are **protected** — they can only be deleted manually (per-item). The following commands enforce this:

| Command | Behaviour |
|---------|-----------|
| `clear_all_clips` | `DELETE … WHERE folder_id IS NULL` — never touches folder items |
| `clear_clipboard_history` | `DELETE … WHERE is_deleted = 1 AND folder_id IS NULL` — skips soft-deleted folder items |
| `remove_duplicate_clips` | Dedup query scoped to `folder_id IS NULL` in both outer DELETE and inner SELECT MIN(id) — folder items are never removed |

> **Rule**: Any future bulk-delete or auto-trim logic (e.g. `max_items` enforcement, `auto_delete_days`) **must** include `AND folder_id IS NULL` to preserve folder contents.
