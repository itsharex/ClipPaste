# ClipPaste — Project Memory

## Project Overview
- **Product**: ClipPaste — clipboard history manager for Windows
- **Stack**: Tauri v2 (Rust) + React 18 + TypeScript + Vite + TailwindCSS v3 + SQLite (sqlx 0.7)
- **Current version**: 1.1.7
- **Package name**: `clippaste`
- **Communication**: User speaks Vietnamese; all files must be written in English

## Version Bump Checklist
When bumping version, update ALL 3 files:
1. `package.json` → `"version"`
2. `src-tauri/Cargo.toml` → `version`
3. `src-tauri/tauri.conf.json` → `"version"`

## Build & Dev Commands
```bash
pnpm tauri dev       # Development
pnpm tauri build     # Production build
pnpm format          # Prettier format frontend/src/**
```

## Release Workflow (user preference)
Before building: update `CHANGELOG.md` and bump version in all 3 files.
CHANGELOG format: Keep a Changelog + Semantic Versioning.

## Key File Locations
- Frontend root: `frontend/src/App.tsx` — manages all state
- Clip list UI: `frontend/src/components/ClipList.tsx`
- Keyboard shortcuts: `frontend/src/hooks/useKeyboard.ts`
- Control bar (search + folders): `frontend/src/components/ControlBar.tsx`
- Rust commands: `src-tauri/src/commands.rs`
- Window animation + tray + hotkey: `src-tauri/src/lib.rs`
- Clipboard monitor: `src-tauri/src/clipboard.rs`
- DB layer: `src-tauri/src/database.rs`

## Architecture: Window Open/Reset Flow
Triggered by `tauri://focus` event in `App.tsx`:
1. `setSelectedClipId(null)` — clear stale selection immediately
2. `setSearchQuery('')` — clear search (triggers `loadClips` via useEffect)
3. `autoSelectFirstOnNextLoadRef.current = true` — flag to auto-select after load
4. `setWindowFocusCount(c => c + 1)` — triggers scroll reset + input focus

After `loadClips` completes (non-append), if `autoSelectFirstOnNextLoadRef = true`:
- Set `selectedClipId = data[0]?.id` (first clip from the freshly loaded full list)
- Reset ref to `false`

**Why `useEffect([windowFocusCount])` for focus instead of `setTimeout`:**
Ensures React has re-rendered with `searchQuery = ""` before focusing the input,
preventing the "stale DOM value" bug where the input still contains the old search text.

## Architecture: ClipList Scroll Reset
- Prop `resetScrollKey?: number` passed from `App` → `ClipList`
- `useEffect([resetScrollKey])`: sets `containerRef.current.scrollLeft = 0`
- Triggered by `windowFocusCount` incrementing on window open

## Architecture: Clip List Sort Order
- Sorted by `created_at DESC` — newest copied clip appears first (stable order)
- **Paste does NOT reorder**: only updates `last_pasted_at`, never `created_at`
- **Re-copy bumps to top**: duplicate hash detection → updates `created_at` → moves to top

## Known Bug Patterns & Fixes
| Bug | Root Cause | Fix |
|-----|-----------|-----|
| Arrow nav starts from pasted clip position after reopen | `clipsRef.current[0]` was from filtered search list, not full list | Set `selectedClipId = null` on focus; auto-select `data[0]` after `loadClips` |
| Backspace types in stale input after reopen | `setTimeout(focus)` ran before React re-rendered cleared value | Use `useEffect([windowFocusCount])` to focus after render cycle |
| Ctrl+F doesn't focus search when bar already visible | `setShowSearch(true)` is no-op if already true; `autoFocus` only fires on mount | Always call `searchInputRef.current?.focus()` in the `onSearch` handler |
| Arrow nav on stale list during reload | User presses key before `loadClips` finishes; old filtered clips used | Guard `if (isLoading) return` in `onNavigateUp/Down` |

## Important Constants
- `WINDOW_HEIGHT`: `330.0` (Rust physical px) ↔ `298` (TS logical px) — must stay in sync
- `auto_paste` uses **Shift+Insert** (not Ctrl+V) to avoid clipboard conflicts
- Clipboard dedup: SHA256 hash; re-copy bumps `created_at`; paste only updates `last_pasted_at`

## Settings Keys
| Key | Default | Description |
|-----|---------|-------------|
| `hotkey` | `Ctrl+Shift+V` | Global shortcut |
| `theme` | `dark` | `light` / `dark` / `system` |
| `mica_effect` | `clear` | `clear` / `mica` / `mica_alt` |
| `auto_paste` | `true` | Auto-paste on clip select |
| `max_items` | `1000` | Max clips stored |
| `ignore_ghost_clips` | `false` | Ignore unknown source apps |

## Data Paths (Windows)
- DB: `%APPDATA%/ClipPaste/clipboard.db` (or custom path from `config.json`)
- Config: `%APPDATA%/ClipPaste/config.json`
