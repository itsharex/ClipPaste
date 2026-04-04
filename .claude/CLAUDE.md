# ClipPaste – CLAUDE.md

## Project Overview

**ClipPaste** is a cross-platform clipboard history manager for **Windows and Linux**, built with **Tauri v2** (Rust backend) + **React/TypeScript** (frontend). Package name: `clippaste`, version: `1.7.3`.

### Platform Support

| Feature | Windows | macOS | Linux |
|---------|---------|-------|-------|
| Clipboard monitoring | ✅ | ✅ | ✅ |
| Auto-paste | ✅ (Shift+Insert) | ✅ (Cmd+V via CGEvent) | ❌ |
| Source app detection | ✅ (Win32 API) | ✅ (NSWorkspace) | ❌ |
| Source app icon | ✅ (Win32 API) | ❌ | ❌ |
| Drag-copy to external apps | ✅ (HTML5 Drag) | ✅ (HTML5 Drag) | ✅ (HTML5 Drag) |
| Window effects | Mica/Mica Alt/Clear | Vibrancy | ❌ |
| File/folder picker | PowerShell | osascript | zenity |

Platform-specific code is gated behind `#[cfg(target_os = "...")]`. macOS auto-paste requires Accessibility permission. Source app info captured before clipboard debounce for accuracy.

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
│   ├── App.tsx            # Root component, orchestrates hooks & components
│   ├── components/        # UI components
│   │   ├── ClipList.tsx   # Clipboard item list (virtual scroll)
│   │   ├── ClipCard.tsx   # Card for each clip (search highlight, timestamp, index badge)
│   │   ├── ControlBar.tsx # Control bar (search, folders, settings)
│   │   ├── ContextMenu.tsx
│   │   ├── FolderModal.tsx
│   │   ├── EditClipModal.tsx
│   │   ├── NoteModal.tsx
│   │   ├── ConfirmDialog.tsx
│   │   ├── SettingsPanel.tsx
│   │   └── settings/      # Settings tab components
│   │       ├── GeneralTab.tsx
│   │       ├── FoldersTab.tsx
│   │       ├── DashboardTab.tsx
│   │       └── HotkeysTab.tsx
│   ├── hooks/
│   │   ├── useKeyboard.ts    # Keyboard shortcuts (Esc, Ctrl+F, arrows, Enter, E, P, Ctrl+Delete)
│   │   ├── useTheme.ts       # Theme management
│   │   ├── useClipActions.ts  # Clip CRUD, paste, copy, pin, note
│   │   ├── useFolderActions.ts # Folder CRUD, reorder, move clip
│   │   ├── useDragDrop.ts     # Drag-and-drop between folders
│   │   ├── useFolderPreview.ts # Folder hover preview with cache
│   │   ├── useContextMenu.ts  # Right-click context menu state
│   │   ├── useFolderModal.ts  # Create/rename folder modal state
│   │   └── useBatchActions.ts # Bulk delete, move, paste operations
│   ├── utils.ts           # Shared helpers (base64ToBlob)
│   ├── windows/
│   │   └── SettingsWindow.tsx
│   ├── types/index.ts     # TypeScript types
│   └── constants.ts       # Layout constants (WINDOW_HEIGHT=298, sync with Rust)
│
├── src-tauri/src/         # Rust backend
│   ├── main.rs            # Entry point (calls run_app())
│   ├── lib.rs             # run_app(), window animation, tray, hotkey setup
│   ├── commands/          # Tauri commands (split by domain)
│   │   ├── mod.rs         # Re-exports all command modules
│   │   ├── clips.rs       # get/paste/copy/delete/search/pin/note/bulk_delete/bulk_move
│   │   ├── folders.rs     # get/create/delete/rename/move/reorder
│   │   ├── settings.rs    # get/save settings, ignored apps, hotkey, cleanup
│   │   ├── data.rs        # export/import, dashboard, timeline, file/folder picker
│   │   │   ├── window.rs      # show/hide/focus, dragging, ping, incognito toggle
│   │   └── helpers.rs     # clip_to_item_async, check_auto_paste_and_hide, clipboard_write_text
│   ├── clipboard.rs       # Clipboard monitoring, caches, sensitive detection, incognito mode
│   ├── database.rs        # SQLite pool + migrations (v1-v5)
│   ├── models.rs          # Rust structs (Clip, Folder, ClipboardItem, etc.)
│   ├── constants.rs       # WINDOW_HEIGHT=330.0, WINDOW_MARGIN=0.0
│   ├── utils.rs           # Path helpers (config, data dir)
│   └── tests.rs           # 88 unit + integration tests
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
       subtype, note, paste_count, is_pinned, is_sensitive,
       created_at, last_accessed, last_pasted_at)
folders (id, name, icon, color, is_system, position, created_at)
settings (key TEXT PK, value TEXT)
ignored_apps (id, app_name UNIQUE)
```

## Tauri Commands (invoked from frontend)

```
get_clips, get_clip, get_initial_state, paste_clip, copy_clip, delete_clip, search_clips
get_folders, create_folder, rename_folder, delete_folder, move_to_folder
get_settings, save_settings
get_clipboard_history_size, clear_clipboard_history, clear_all_clips, remove_duplicate_clips
register_global_shortcut, show_window, hide_window, focus_window
add_ignored_app, remove_ignored_app, get_ignored_apps
pick_file, pick_folder, get_layout_config
get_data_directory, set_data_directory
set_dragging, reorder_folders, toggle_pin, paste_text
bulk_delete_clips, bulk_move_clips
export_data, import_data, get_dashboard_stats, get_clips_by_date, get_clip_dates
update_note, toggle_incognito, get_incognito_status, ping, test_log
```

## Core Flows

1. **Clipboard monitoring**: `clipboard.rs::init()` → listens to `plugin:clipboard-x://clipboard_changed` → captures source app info immediately → debounce 150ms → `process_clipboard_change()` → saves to DB → emits `clipboard-change` event
2. **Paste clip**: Frontend invokes `paste_clip` → backend stops listener → writes clipboard → animates window hide → callback: `send_paste_input()` (Shift+Insert on Windows, Cmd+V on macOS)
3. **Window show/hide**: Slide animation from bottom of screen, 15 steps × 10ms. Monitor detected by cursor position (Windows Win32 API). `IS_DRAGGING` flag prevents auto-hide during external drag operations
4. **Window effects (Windows)**: Mica / Mica Alt (Tabbed) / Acrylic / Blur / Clear, using `window-vibrancy` fork
5. **Search**: Client-side pre-filter (instant) + backend LIKE query (skip image BLOBs, 2000-char text_preview). Debounce 80ms. Generation counter discards stale responses
6. **Drag-copy**: HTML5 Drag API — cards are `draggable`, `dataTransfer` carries text/plain or image file. Works for both internal folder moves and external app drops

## Settings

| Key | Default | Description |
|-----|---------|-------------|
| `hotkey` | `Ctrl+Shift+V` | Global shortcut to open the app |
| `theme` | `dark` | `light` / `dark` / `system` |
| `mica_effect` | `clear` | `clear` / `mica` / `mica_alt` / `acrylic` / `blur` |
| `auto_paste` | `true` | Auto-paste after selecting a clip |
| `max_items` | `0` | Max clips to store (0 = unlimited) |
| `auto_delete_days` | `0` | Auto-delete clips older than N days (0 = disabled) |
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
- `auto_paste` uses Shift+Insert on Windows (not Ctrl+V to avoid conflicts), Cmd+V via CGEvent on macOS (requires Accessibility permission)
- Settings window (`label: 'settings'`) is a separate WebviewWindow, URL: `index.html?window=settings`
- Clipboard dedup: uses SHA256 hash of content; if hash exists, bumps `created_at` (re-copy moves to top) and restores if soft-deleted
- Paste a clip updates `last_pasted_at` only — does NOT bump `created_at`, so pasting never reorders the list
- List sort order: `created_at DESC` (newest copy first, stable — paste does not change order)
- `ClipCard` has `data-clip-id={clip.id}` for DOM lookup; `ClipList` auto-scrolls selected card into view on `selectedClipId` change
- `CLIPBOARD_SYNC` mutex: prevents conflicts between clipboard monitor and paste operations
- `IS_ANIMATING` atomic flag: prevents race conditions during simultaneous show/hide
- `IS_DRAGGING` atomic flag: prevents window auto-hide during HTML5 drag to external apps
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

## Release Checklist

**NEVER re-tag the same version.** Users on that version won't receive the auto-update (Tauri updater compares version strings — same version = no update). Always bump to a new version for hotfixes.

1. Update version in **all 3 files** (must match):
   - `src-tauri/tauri.conf.json` → `version`
   - `src-tauri/Cargo.toml` → `version`
   - `package.json` → `version`
2. Update `CHANGELOG.md` — add new section under `[Unreleased]`
3. Update `.claude/CLAUDE.md` — version in Project Overview
4. Commit all changes
5. `git tag vX.Y.Z` — tag the commit
6. `git push origin main vX.Y.Z` — push commit + tag (triggers CI)
7. Wait for CI — all 4 jobs must pass (Windows ×2, Linux ×1 + create-release)
8. If CI fails → fix, bump version again (e.g. v1.4.6 → v1.4.7), repeat from step 1
