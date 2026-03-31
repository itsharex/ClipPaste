# Changelog

All notable changes to ClipPaste will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

---

## [Unreleased]

---

## [1.4.8] - 2026-03-31

### Added
- **Image storage on disk** — images saved as `{hash}.png` in `images/` directory instead of BLOB in DB, with auto-migration from old format and VACUUM to reclaim space
- **Subtype detection** — auto-detect URL, email, color code, file path when copying; displayed with visual badges and styled cards
- **Note for clips** — add/edit notes on any clip via right-click context menu (in-app modal, no window.prompt)
- **Paste count** — tracks how many times each clip is pasted, shown as ×N on card footer
- **Virtual horizontal list** — `@tanstack/react-virtual` renders only visible cards (~5-6 DOM elements regardless of list size)
- **Multi-word search** — "docker compose" matches clips containing both "docker" AND "compose" anywhere
- **Export/Import backup** — zip archive containing `clipboard.db` + `images/` folder, accessible from Settings
- **Clip saved toast** — visual confirmation when a new clip is captured
- **Auto-delete old clips** — enforces `max_items` setting on startup, removes oldest non-folder clips
- **Orphan image cleanup** — removes disk images without matching DB record on startup

### Changed
- **DB config reverted to SQLite defaults** — DELETE journal mode + synchronous Full (no WAL), matching Ditto/CopyQ/Maccy
- **Context menu simplified** — removed "Move to folder" options (drag-and-drop handles this)
- **Card footer redesigned** — shows character count / image size + paste count

### Performance
- **DB size reduced** — image BLOBs removed from DB, file typically shrinks from ~40MB to ~2MB after VACUUM
- **Virtual list** — constant DOM element count regardless of clip history size; smooth scroll with 10,000+ clips

---

## [1.4.7] - 2026-03-31

### Added
- **Skeleton loading cards** — animated placeholder cards with shimmer effect and staggered entrance while search results load, replacing blank/frozen screen
- **"No results" message** — shows clear feedback when search finds no clips instead of infinite skeleton
- **Search within folder** — clicking a folder then searching filters within that folder only

### Fixed
- **Search no longer flashes old clips** — clips are cleared immediately when typing, skeleton shows instantly
- **Hover folder no longer interferes with search** — folder preview is cancelled when search begins
- **Window reopen resets to All** — reopening app via hotkey always returns to All folder with cleared search
- **Search no longer floods backend** — 150ms debounce + generation counter prevents stale results

### Performance
- **In-memory search cache** — all clip previews loaded into RAM at startup (~100KB for 558 clips), search filters in-memory in <1ms instead of 1.5s SQLite scan
- **SQLite WAL mode** — enabled Write-Ahead Logging for faster concurrent reads
- **Search skips content BLOB entirely** — SQL query only fetches lightweight columns (no 26KB+ content), search results use `text_preview` for display
- **Search 300x faster** — from 1500ms (SQLite LIKE full-table scan) to ~5ms (in-memory filter + UUID index lookup)

### Changed
- **Search results show text_preview** — cards display 2000-char preview instead of full content for faster rendering (full content loaded on paste)

---

## [1.4.6] - 2026-03-31

Version bump for auto-updater (v1.4.5 was re-tagged, users on old 1.4.5 wouldn't receive the hotfix).

---

## [1.4.5] - 2026-03-31

### Fixed
- **Search returning wrong results** — removed FTS5 (was matching unrelated clips), reverted to optimized LIKE search (skip image BLOBs, 2000-char preview)

### Performance
- **Instant search pre-filter** — client-side filtering of visible clips while waiting for backend results, zero perceived latency
- **Search debounce 200ms → 80ms** — faster response with optimized queries
- **Stale query discard** — generation counter prevents old search results from overwriting newer ones

### Changed
- **Stagger entrance animation** — clips slide in from left to right when switching folders or reopening window
- **No animation on search** — search results appear instantly without fade/stagger to avoid flicker

---

## [1.4.4] - 2026-03-31

### Added
- **macOS source app detection** — uses `NSWorkspace.frontmostApplication` to identify which app the user copied from, with app name and bundle ID
- **macOS source app icon** — extracts app icon via `NSWorkspace.iconForFile` and converts to PNG base64 for display in clip cards
- **macOS auto-paste (Cmd+V)** — uses `core-graphics` CGEvent to simulate Cmd+V after selecting a clip (requires Accessibility permission)
- **Drag-copy to external apps** — drag any clip card out of the window to drop text/images into other applications (HTML5 Drag API, cross-platform)
- **Drag-to-folder** — folder tabs now accept HTML5 drag drops in addition to the existing move workflow

### Fixed
- **Search state persisting on reopen** — fixed race condition where stale `searchQuery` closure caused old search results to flash when reopening the app via hotkey
- **Source app capture timing** — clipboard owner info is now captured before the 150ms debounce (not after), preventing wrong app detection when user switches apps quickly

### Changed
- **Unified drag system** — replaced dual mouse-based + HTML5 drag with a single HTML5 Drag API system for both internal folder moves and external drag-copy
- **Window auto-hide suppression** — app window stays visible during drag operations to external apps (`IS_DRAGGING` flag)

### Performance
- **FTS5 full-text search** — SQLite FTS5 virtual table for near-instant text search (~1ms vs 100ms+ LIKE scan)
- **Skip image BLOB scan** — search no longer scans image binary data, only text clips
- **Expanded text_preview** — increased from 200 to 2000 characters; existing clips auto-migrated on startup
- **No search flicker** — loading spinner only shows on initial load, not during search/refresh

---

## [1.4.3] - 2026-03-30

### Added
- **macOS support** — `.dmg` builds for Apple Silicon (M1+) and Intel
- **Linux support** — `.deb`, `.AppImage`, and `.rpm` builds for x64
- **Cross-platform CI/CD** — GitHub Actions now builds for Windows, macOS, and Linux (5 targets)

### Changed
- **New app icon** — redesigned SVG-based vector icon with gradient clipboard, cute face, sparkles, and transparent background; crisp at all sizes from 32×32 to 512×512
- **Platform-conditional code** — `windows` crate only compiles on Windows; clipboard owner detection, icon extraction, and auto-paste gracefully degrade on non-Windows
- **File/folder pickers** — native dialogs per platform (PowerShell on Windows, osascript on macOS, zenity on Linux)
- **window-vibrancy** — split dependency: custom fork for Windows (Mica/Tabbed/rounded corners), upstream v0.5 for macOS (vibrancy), no-op for Linux

---

## [1.3.9] - 2026-03-30

### Added
- **Card hover animation** — subtle scale(1.02), lift(-3px), and tilt(-0.5deg) on hover with primary color glow shadow
- **Search debounce** — 200ms debounce on search input to prevent flickering during typing

### Changed
- **Selected card** — enhanced: scale(1.04), lift, stronger blue glow shadow
- **Hover ring** — brighter primary/40 ring on hover for better visual feedback

---

## [1.3.8] - 2026-03-30

### Fixed
- **Search full content** — search now works on full clip text, not just first 200 chars (`CAST(content AS TEXT)` for BLOB columns)
- **Folder name validation** — allow `/` in folder names (was silently blocking "SSL / TLS", "Network / Misc" from saving)
- **Modal Save button** — render via React Portal to fix click events being blocked by `overflow-hidden` parent
- **Content type filter** — smart detection: URL (`http://...`), file path (`C:\...`) detected from text content

### Added
- **56 colored icons** — 20 new icons: Laptop, Monitor, PC, Wifi, Router, GitBranch, GitHub, Package, Workflow, Gauge, Cog, Cable, Plug, Activity, Hash, ShieldCheck, LockKeyhole, AppWindow, RefreshCw, Blocks — each with distinct color
- **Delete folder confirmation** — `window.confirm` dialog before deleting a folder

### Changed
- **Context menu** — merged "Rename" and "Change color" into single "Edit folder" option

---

## [1.3.7] - 2026-03-30

### Added
- **Folder icons** — 20 icon options (Briefcase, Code, Bookmark, Lock, Star, Heart, Zap, Coffee, etc.) when creating or editing folders; icons display alongside folder name in tabs
- **Content type filter** — filter clips by type (Text, Image, HTML, RTF, File, URL) using icon buttons next to the search bar
- **Folder drag reorder** — grab and drag folder tabs to reorder them (simulated drag, works reliably on Windows/Tauri)
- **Folder move via context menu** — right-click folder tab for "Move to start", "Move left" (repeatable), "Move right" (repeatable), "Move to end"
- **Drag ghost preview** — floating folder tab follows cursor while dragging, showing folder name, icon, and color
- **"All" tab icon** — Layers icon on the system "All" folder tab for visual consistency

### Changed
- **Visual polish: toolbar** — gradient background with backdrop blur on the control bar
- **Visual polish: window shadow** — upgraded to directional shadow (`0 4px 32px`) for depth
- **Visual polish: font** — Segoe UI Variable (Windows 11 native) with Inter fallback for modern feel
- **Visual polish: borders** — softer border opacity (`border-border/50`) on toolbar
- **Hover effect** — subtle `-translate-y-2px` lift on card hover with CSS transitions (replaced Framer Motion spring to eliminate jitter)
- **Ctrl+P** — now triggers pin/unpin instead of opening browser print dialog
- **Reduced bundle size** — removed heavy Framer Motion card animations in favor of lightweight CSS transitions (405KB → 279KB JS)

### Fixed
- **Clipboard listener going stale** — after prolonged use, new clips would not appear at the top; root cause was unstable listener re-subscriptions due to dependency chain (`clips.length` → `loadClips` → `refreshCurrentFolder` → listener`). Fixed by using stable refs so the listener subscribes once and never re-subscribes
- **Window focus reload** — force-reloads clips on every window focus to guarantee fresh data, also resets content type filter
- **Folder animation jitter** — switching folders no longer causes cards to flash/shake; replaced per-card entrance animation with a subtle container crossfade

### Backend
- **`rename_folder` now accepts `icon` parameter** — saves folder icon to database alongside name and color

---

## [1.3.6] - 2026-03-29

### Performance
- **Memoized gradient colors** — card header colors cached, no recalculation per render
- **Memoized folder colors** — only recomputed on theme change
- **Debounced folder refresh** — rapid copies only trigger 1 DB call instead of N
- **Smarter preview cache** — only clears when clips are added/removed, not on count changes

---

## [1.3.5] - 2026-03-29

### Fixed
- **Reverted smooth scroll** — momentum scroll caused wheel, search, and Ctrl+F to break; restored original simple scroll

---

## [1.3.4] - 2026-03-29

### Added
- **Smooth scroll with momentum** — clip list now scrolls with inertia and friction, buttery smooth horizontal scrolling using requestAnimationFrame (zero extra dependencies)

---

## [1.3.3] - 2026-03-29

### Fixed
- **Arrow keys in search** — arrow up/down now navigate clips while search input is focused

---

## [1.3.2] - 2026-03-29

### Added
- **Per-folder pin** — pin only affects the folder it belongs to; "All" view ignores pin status
- **Search filters folders** — typing in search bar filters both clips and folder tabs simultaneously
- **Gradient card headers** — headers now use gradient colors instead of flat backgrounds
- **Update progress bar** — shows percentage, downloaded/total MB, and animated progress bar during auto-update

### Changed
- **Smoother card animations** — transitions target specific properties (transform, shadow) instead of `transition-all` for better performance
- **Selected card glow** — subtle blue shadow glow when a card is selected
- **Snappier button transitions** — pin/copy buttons use 150ms opacity transition
- **Native rounded corners** — Windows 11 DWM API for native-looking window corners
- **Flicker-free effect switching** — `switch_effect()` clears old effect before applying new one
- **Smart OS fallback** — Mica falls back to Acrylic on Windows 10, Tabbed falls back to Mica then Acrylic

### Fixed
- **Settings X button** — no longer triggers window maximize instead of close
- **Footer year** — corrected from 2025 to 2026

---

## [1.2.9] - 2026-03-28

### Added
- **Update progress bar** — shows percentage, downloaded/total MB, and animated progress bar during auto-update download

### Fixed
- **Footer year** — corrected from 2025 to 2026

---

## [1.2.8] - 2026-03-28

### Changed
- **Window vibrancy upgraded** — using custom `window-vibrancy` v0.8.0 with flicker-free effect switching (`switch_effect`), smart OS-version fallback, and `clear_all_effects`
- **Native rounded corners** — Windows 11 DWM `DWMWA_WINDOW_CORNER_PREFERENCE` for native-looking window corners
- **Smoother effect transitions** — switching between Mica/Mica Alt/Clear no longer flickers

---

## [1.2.7] - 2026-03-28

### Added
- **Folder hover preview** — hover over a folder tab to instantly preview its clips in the main list without switching folders; move mouse down to interact (select, paste, copy), move away to return to current folder
- **Pin/Unpin clips** — pin important clips to the top of the list; toggle via pin icon on card header, keyboard shortcut `P`, or right-click context menu
- **Winget manifest** — prepared manifest files for `winget install Phieu-Tran.ClipPaste`

---

## [1.2.6] - 2026-03-16

### Fixed
- **Security: CSP enabled** — added Content Security Policy (`script-src 'self'`, `img-src 'self' data:`) to prevent XSS
- **Security: path traversal blocked** — `set_data_directory` now rejects relative paths, `..` traversal, and UNC/network paths
- **Security: sensitive data removed from logs** — clipboard content preview, hashes, and exe paths are no longer logged
- **Security: COM resource leak fixed** — `CoUninitialize` is now always called in `pick_folder` regardless of error path
- **Silent DB errors fixed** — clipboard insert/update failures are now logged and no longer emit misleading frontend events
- **Folder name validation** — reject names longer than 50 characters or containing special characters (`<>:"|?*\/`)
- **Arrow key navigation in search** — arrow keys no longer hijack cursor movement while typing in the search bar
- **Config serialization panic fixed** — replaced `.unwrap()` with proper error handling in `set_data_directory`

---

## [1.2.5] - 2026-03-10

### Fixed
- **Folder items protected from "Clear History"**: bulk clear operations now correctly preserve all clips saved in user folders (`folder_id IS NULL` filter enforced in `clear_all_clips`, `clear_clipboard_history`, and `remove_duplicate_clips`)
- **Deleting a folder now removes its clips**: previously deleting a folder left its clips as orphaned DB rows that were invisible but permanently shielded from any bulk-delete — now the clips are hard-deleted together with the folder
- **Main window refreshes after Clear History**: `clear_all_clips` now emits `clipboard-change` so the main window reloads immediately without requiring a new clipboard copy
- **Folder item delete is now a hard-delete**: deleting a clip that lives inside a folder performs a hard-delete instead of soft-delete, preventing uncleanable soft-deleted orphan rows

---

## [1.2.4] - 2026-03-09

### Fixed
- **Window stuck visible after closing settings**: fixed a race condition where closing the settings window while the main window's blur event was suppressed caused the main window to remain visible permanently. Now detects settings window destruction and hides main window if needed
- **IS_ANIMATING flag could get stuck**: replaced manual `store(false)` calls with a RAII guard so the animation lock is always released even if the animation thread panics

---

## [1.2.3] - 2026-03-07

### Added
- **Folder color picker**: choose a color for each folder when creating or renaming — right-click a folder tab and select "Change color" or pick a color during creation
- Folder color is persisted to the database and reflected on the folder tab in the main window

### Fixed
- **Folder tab scroll**: scrolling up (left) on the folder tab bar now works correctly — mouse wheel up/down is properly mapped to horizontal scroll

---

## [1.2.2] - 2026-03-05

### Fixed
- Minor stability improvements

---

## [1.2.1] - 2026-03-04

### Fixed
- **Edit hotkey**: edit shortcut (`E`) no longer fires while typing in the search bar or any input field
- **Folder picker**: refactored to use Windows COM API directly instead of PowerShell, improving reliability and speed

---

## [1.2.0] - 2026-03-03

### Added
- **Edit before paste**: press `E` on a selected clip to open an editor and modify the text before pasting — images are excluded

---

## [1.1.9] - 2026-03-01

### Fixed
- **Multi-monitor support**: fixed wrong monitor detection on setups with different DPI scales — `get_monitor_at_cursor` now uses Win32 `MonitorFromPoint` API instead of manual coordinate comparison
- **Stacked monitors (top/bottom)**: window no longer briefly appears on the lower monitor during slide animation — animation is skipped when a monitor is detected below

---

## [1.1.8] - 2026-02-27

### Added
- **Folder reordering**: drag a folder tab and drop it onto another to rearrange the order — persisted to database so it survives restarts
- **Folder tab auto-scroll**: when a folder is selected, the tab bar now smoothly scrolls to keep it visible even when many folders exist

### Changed
- Delete shortcut changed from `Delete` to **`Ctrl+Delete`** to prevent accidental clip deletion

### Fixed
- Rapid `Ctrl+Delete` presses no longer cause duplicate delete errors — concurrent delete calls are now properly guarded

---

## [1.1.7] - 2026-02-25

### Changed
- Clip list order is now stable: sorted by **copy time** (`created_at DESC`) — newest copy always appears first
- Pasting a clip no longer bumps it to the top of the list (position stays where it was originally copied)
- Re-copying an existing clip bumps it back to the top (as expected)
- When the app is opened via hotkey, the clip list now **always resets to the beginning** (first/newest clip)
- Search query is automatically cleared each time the app is opened via hotkey

### Fixed
- Arrow key navigation (Up/Down) now auto-scrolls the clip list to keep the selected card visible
- `Ctrl+F` now correctly focuses the search input even when the search bar is already visible
- After searching and pasting, reopening the app no longer resumes navigation from the previously pasted clip's position — arrow keys now start from the first clip
- Arrow key navigation is disabled while the clip list is loading to prevent navigating on a stale list

---

## [1.1.6] - 2026-02-25

### Added
- Data directory management: users can now choose a custom folder to store the database
- Folder picker dialog via PowerShell (`pick_folder` command)
- Config file (`config.json`) persists the custom data directory path across restarts
- Auto-migration of `clipboard.db` when data directory is changed

### Changed
- Renamed product and all internal references from "Clipboard" to "ClipPaste"

### Fixed
- Hotkey listener now correctly re-registers after app restart
- Hotkey setting persisted to database so it survives restarts

---

## [1.1.5] - 2025-xx-xx

### Changed
- Refactored paste architecture: image writing is now handled by the frontend (navigator.clipboard API) to avoid OS threading issues

### Fixed
- Auto-paste now uses **Shift+Insert** instead of Ctrl+V to prevent clipboard conflicts
- Hotkey setting correctly persisted to database across restarts

---

## [1.1.4] - 2025-xx-xx

### Changed
- Replaced `tauri-plugin-clipboard` with `tauri-plugin-clipboard-x` to fix image clipboard writes on Windows (`OSError 1418: Thread does not have a clipboard open`)

---

## [1.1.3] - 2025-xx-xx

### Added
- Mica Alt (Tabbed) window effect option for a more modern look on Windows 11
- `mica_effect` setting: `clear` / `mica` / `mica_alt`

### Changed
- Refined Mica effect application logic

---

## [1.1.2] - 2025-xx-xx

### Changed
- UI refinements: adjusted font size and padding inside clip cards

---

<!-- Links -->
[Unreleased]: https://github.com/Phieu-Tran/ClipPaste/compare/v1.2.6...HEAD
[1.2.6]: https://github.com/Phieu-Tran/ClipPaste/compare/v1.2.5...v1.2.6
[1.2.5]: https://github.com/Phieu-Tran/ClipPaste/compare/v1.2.4...v1.2.5
[1.2.4]: https://github.com/Phieu-Tran/ClipPaste/compare/v1.2.3...v1.2.4
[1.2.3]: https://github.com/Phieu-Tran/ClipPaste/compare/v1.2.2...v1.2.3
[1.2.2]: https://github.com/Phieu-Tran/ClipPaste/compare/v1.2.1...v1.2.2
[1.2.1]: https://github.com/Phieu-Tran/ClipPaste/compare/v1.2.0...v1.2.1
[1.2.0]: https://github.com/Phieu-Tran/ClipPaste/compare/v1.1.9...v1.2.0
[1.1.9]: https://github.com/Phieu-Tran/ClipPaste/compare/v1.1.8...v1.1.9
[1.1.8]: https://github.com/Phieu-Tran/ClipPaste/compare/v1.1.7...v1.1.8
[1.1.7]: https://github.com/Phieu-Tran/ClipPaste/compare/v1.1.6...v1.1.7
[1.1.6]: https://github.com/Phieu-Tran/ClipPaste/compare/v1.1.5...v1.1.6
[1.1.5]: https://github.com/Phieu-Tran/ClipPaste/compare/v1.1.4...v1.1.5
[1.1.4]: https://github.com/Phieu-Tran/ClipPaste/compare/v1.1.3...v1.1.4
[1.1.3]: https://github.com/Phieu-Tran/ClipPaste/compare/v1.1.2...v1.1.3
[1.1.2]: https://github.com/Phieu-Tran/ClipPaste/releases/tag/v1.1.2
