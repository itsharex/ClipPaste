# Changelog

All notable changes to ClipPaste will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

---

## [Unreleased]

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
