# Changelog

All notable changes to ClipPaste will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

---

## [Unreleased]

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
[Unreleased]: https://github.com/Phieu-Tran/ClipPaste/compare/v1.1.9...HEAD
[1.1.9]: https://github.com/Phieu-Tran/ClipPaste/compare/v1.1.8...v1.1.9
[1.1.8]: https://github.com/Phieu-Tran/ClipPaste/compare/v1.1.7...v1.1.8
[1.1.7]: https://github.com/Phieu-Tran/ClipPaste/compare/v1.1.6...v1.1.7
[1.1.6]: https://github.com/Phieu-Tran/ClipPaste/compare/v1.1.5...v1.1.6
[1.1.5]: https://github.com/Phieu-Tran/ClipPaste/compare/v1.1.4...v1.1.5
[1.1.4]: https://github.com/Phieu-Tran/ClipPaste/compare/v1.1.3...v1.1.4
[1.1.3]: https://github.com/Phieu-Tran/ClipPaste/compare/v1.1.2...v1.1.3
[1.1.2]: https://github.com/Phieu-Tran/ClipPaste/releases/tag/v1.1.2
