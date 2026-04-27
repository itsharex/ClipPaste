# Changelog

All notable changes to ClipPaste will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

---

## [1.10.8] - 2026-04-27

### Added
- **Ignored apps now also block hotkey + auto-paste**: previously the Ignored Applications list only suppressed clipboard recording from the named app, but the global hotkey still opened ClipPaste over a fullscreen game and auto-paste still fired keystrokes into it. Now, when the foreground window belongs to an ignored app, `Ctrl+Shift+V` (main) and `Ctrl+Shift+S` (scratchpad) are silently swallowed and `Shift+Insert` is not sent — the app stays completely invisible to that process. Match logic also accepts entries with or without `.exe`, and falls back to extracting the filename from the full path for apps like PUBG / Valorant whose anti-cheat blocks `GetModuleBaseNameW` (so a foreground exe of `None` with `path=…\TslGame.exe` still matches an entry of `TslGame`).

---

## [1.10.7] - 2026-04-26

### Added
- **Inspect folder contents from Settings → Folders**: each folder row is now expandable inline, revealing the clips inside (preview, type icon, note, relative timestamp). Per-clip hover actions to **Move** (with searchable folder picker) or **Delete**. Built for users with many folders who need to audit which clips landed where without leaving Settings. Lazy-loaded with `previewOnly: true` so image BLOBs aren't decoded.
- **Smart virtual category**: new "Smart" pill in the category bar alongside "Frequent". Ranks clips by `paste_count` weighted by recency on `last_pasted_at` (7-day halflife), pinned clips always first, only includes clips with at least one paste. Useful when "Frequent" is dominated by old high-count clips that are no longer relevant.

---

## [1.10.6] - 2026-04-23

### Fixed
- **ESC in search felt "abrupt"**: the `ControlBar` search input had its own ESC handler that called `onSearchClick()`, which cleared the text *and* collapsed the search bar in one keystroke — bypassing the staged ESC priority in `useKeyboard`. It also fired alongside the document-level handler, so the sequence was inconsistent. ESC now flows through a single staged path: (1) clear search text only, keep bar open + focus, (2) close the search bar, (3) deselect clip, (4) exit folder, (5) hide app. One "back step" per press, no more skipping levels.

---

## [1.10.5] - 2026-04-23

### Fixed
- **Stray Alt/Win tap on paste (menu bar flashes in Notepad, focus bounces in SecureCRT)**: `send_paste_input` unconditionally injected KEYUP for every modifier (Ctrl, Alt, LWin, RWin) before the Shift+Insert sequence as a "belt-and-suspenders" cleanup. But on Windows, an orphan KEYUP for Alt or Win — with no preceding KEYDOWN and no other key in between — is treated by the target app as a full press-and-release: Alt activates the menu bar, Win opens the Start menu. Symptom was intermittent because it depended on what the target app did with an unpaired KEYUP. Now we poll `GetAsyncKeyState` for each of Ctrl / Alt / LWin / RWin and only emit KEYUP for modifiers that are actually held. This also matters for users who switch virtual desktops via Ctrl+Win+Arrow, where Win can briefly still register as held when the paste fires.

---

## [1.10.4] - 2026-04-20

### Fixed
- **Scratchpad paste race**: `scratchpad_paste` used to spawn the restore-foreground → Shift+Insert chain via `tauri::async_runtime::spawn` with fixed sleeps, while the frontend concurrently ran `setMode('collapsed')` + `appWindow.show()`. The scratchpad window could reappear and steal foreground before keystrokes reached the target app, causing intermittent paste failures. The chain now runs inline-awaited inside the command; the frontend's `doPaste` only resolves after Shift+Insert has been delivered, then it's safe to re-show the collapsed tab.
- **Ctrl+Enter in paste modal sent nothing**: the user's physical Ctrl was still held when the backend fired Shift+Insert, so target apps saw Ctrl+Shift+Insert (not a paste combo). `send_paste_input` now polls `GetAsyncKeyState` for up to 400 ms for Ctrl/Alt/Win to be physically released, then injects KEYUP for every modifier variant as a belt-and-suspenders before the Shift+Insert sequence.

---

## [1.10.3] - 2026-04-17

### Changed
- **Scratchpad modal resized**: paste/edit modal bumped from 520×420 to 680×520 for more editing room. Content textarea rows 6 → 10.
- **Tighter padding**: header/actions `py-3` → `py-2`, content `p-4` → `p-3`.
- **Text sizing**: content/title inputs use `text-[13px]` (was 14px, briefly 11px — settled on readable middle ground). Section labels `text-[10px]`, button labels `text-xs`.

---

## [1.10.2] - 2026-04-17

### Fixed
- **Scratchpad paste now reaches the target app**: previously, `scratchpad_paste` just hid the panel and fired Shift+Insert, which sometimes landed on the wrong window (Windows didn't always restore the user's previous app as foreground in time, especially for `alwaysOnTop` panels). Now we snapshot `GetForegroundWindow()` via a new `capture_prev_foreground` command (fired on scratchpad hotkey + on mouse-enter of the collapsed tab) and use the `AttachThreadInput` + `SetForegroundWindow` + `BringWindowToTop` trick to reliably re-front the target app before sending keystrokes.
- **Edit modal flicker while typing**: `getCurrentWindow()` returns a fresh proxy on each call, which made the `moveToSide/Center/Collapsed` callbacks unstable. Their dependent `useEffect` re-ran on every React render, triggering a hide→resize→show cycle on every keystroke. Memoized the window handle with `useMemo` — editing is smooth now.

---

## [1.10.1] - 2026-04-17

### Added
- **Target-app picker in Ignored Applications**: crosshair button next to the app input; click it, switch to the app you want to block within the 4-second countdown, and its exe name is captured automatically. No more hunting for process names — useful when you only realize a background app should be ignored after you see its clips appear.
- Backend command `pick_foreground_app(delay_ms)` that sleeps then reads the Win32 foreground window (process name + exe path + display name).

---

## [1.10.0] - 2026-04-17

### Added — Scratchpad UX
- **Global hotkey `Ctrl+Shift+S`** toggles the scratchpad panel (backend-registered, emits `scratchpad-toggle` event)
- **Keyboard nav in list mode**: `↑↓` select, `Enter` paste, `E` edit, `Delete` remove, `/` focus search
- **Undo delete** — sonner toast with 5-second Undo button; recreates the note with original title/content/color
- **Color filter** — dot row in header; click to filter, click again to clear; only shown when any note is colored
- **Sort options** — Manual / A-Z / Recent (pinned still always on top)
- **Markdown preview** — lightweight inline renderer for `**bold**`, `*italic*`, `` `code` ``, and `- bullets`
- **Char counter** — appears on hover at bottom-right of each card
- **Pinned/unpinned divider** — subtle "Others" separator between sections
- **Selected ring** — primary-colored ring highlights the keyboard-selected note
- **Paste strip on left of card** (quick access) + Edit button in hover toolbar

### Added — Backend / Sync
- `DETECTION_RULES_VERSION` const — startup rescan skipped unless code version > DB-stored version (speeds up launch)
- Orphan image cleanup on Drive during compact — prunes `img_{hash}.png` files no longer referenced by any clip
- Network retry with exponential backoff for all Drive API calls (on top of existing 429/503 retry)

### Changed
- DB `max_connections: 1 → 4` — WAL readers no longer block behind writes
- Startup cleanup scans (`enforce_max_items`, `enforce_auto_delete`, `cleanup_orphan_images`) deferred to a background task with 2s delay — UI interactive sooner
- `enforce_max_items` / `enforce_auto_delete` use targeted `remove_from_search_cache(uuid)` instead of rebuilding the whole 50K-entry cache
- Sync protocol: **single `list_files` call at start** replaces 3–4 separate listings (state, ops, legacy deltas, images) — saves quota
- Image uploads run with bounded concurrency of 4 via `futures::buffer_unordered` instead of sequentially
- Scratchpad tab collapsed: 14×80 → 16×100 with minimal 3-dot grip (no more awkward yellow icon)
- Paste/edit modal: hide → resize → show sequence reduces flicker when switching from side panel
- Note color gradient stronger (`0.12/0.04` vs `0.04/0.01`) with thicker 5px left border — notes easier to distinguish
- `scratchpad_paste` backend: `std::thread::spawn` + `std::thread::sleep` → `tauri::async_runtime::spawn` + `tokio::time::sleep` (non-blocking)

### Removed
- `fields_json` column + `TemplateField` type — template feature was never wired to UI; dropped in migration v13

### Fixed
- `tests.rs` updated to reflect schema v13 and scratchpad-shape SyncDelta/SyncState (was failing on missing `scratchpads` field)

### Hotkeys reference
- Updated Settings > Hotkeys tab with 3 sections: General (adds Scratchpad toggle), Clipboard list, Scratchpad
- README rewritten with per-surface shortcut tables

### Tests
- 129 / 129 passing (schema_version bumped 12 → 13, scratchpads test-data backfilled)

---

## [1.9.0] - 2026-04-16

### Added
- **Scratchpad sidebar** — persistent notes panel docked to right edge of screen, independent of clipboard history
  - Auto-starts with ClipPaste as a separate always-on-top window
  - Collapsed: thin 14px hover tab; hover → expands to 300px side panel
  - Click paste on note → centered 520×420 modal for edit-before-paste (Ctrl+Enter to paste)
  - Notes support title, content, color (8 presets), pin-to-top, drag reorder, drag-to-external-apps
  - Search notes by title/content
  - Panel pin: click inside panel → stays open, only closes on X or Esc
  - Aurora gradient background (violet/blue/pink/teal) with depth
  - Full Google Drive sync support (scratchpads included in delta protocol, tombstone propagation)
- DB migrations v8–v12: `scratchpads` table with `uuid`, `title`, `content`, `fields_json`, `is_pinned`, `color`, `position`
- 7 new Tauri commands: `get_scratchpads`, `create_scratchpad`, `update_scratchpad`, `delete_scratchpad`, `reorder_scratchpads`, `toggle_scratchpad_pin`, `scratchpad_paste`

---

## [1.8.6] - 2026-04-10

### Fixed
- **Re-copying a clip self-heals the search cache** — if a clip's entry was missing from the in-memory `SEARCH_CACHE` for any reason, copying the same content again now re-inserts it (with current `folder_id` + `note` from DB), restoring searchability without needing a restart
- **Folder tombstone updates search cache** — when a folder is deleted via sync tombstone from another device, affected clips' cache entries are now patched to `folder_id = None`, preventing mismatches between DB state and cache (folder view + search rank)
- **`enforce_auto_delete` rebuilds search cache** — auto-delete now reloads `SEARCH_CACHE` after trimming old clips, matching `enforce_max_items` behavior; prevents stale UUIDs in the in-memory index

### Added
- **3 regression tests** for the search cache fixes above (self-heal, folder tombstone, auto-delete); total Rust tests: 126 → 129
- New helper `clipboard::refresh_search_cache_for_clip` — reusable cache resync primitive used by the re-copy self-heal path

---

## [1.8.5] - 2026-04-09

### Added
- **33 sync protocol tests** — comprehensive unit tests for `apply_delta` (insert, update, skip, hash dedup, tombstone delete, folder reconciliation, "(synced)" cleanup), `build_full_state`, and SyncReport tracking; total Rust tests: 93 → 126
- **Google Drive API rate limiting** — retry up to 3x on HTTP 429/503 with exponential backoff (1s → 2s → 4s + jitter), respects `Retry-After` header, new `SyncError::RateLimited` variant for UI feedback

### Changed
- **App.tsx refactored** — extracted 3 new hooks (`useWindowLifecycle`, `useSearch`, `useMultiSelect`), reducing App.tsx from 698 → 554 lines; zero behavioral changes
- **Sync protocol visibility** — `SyncState`, `SyncDelta`, `apply_delta`, `build_full_state` now `pub(crate)` for testability

---

## [1.8.4] - 2026-04-09

### Added
- **Search persists across window toggles** — search text stays when reopening the window, only clears on Esc
- **Paste bumps clip to top** — pasting a clip now moves it to the top of the list (updates `created_at`)
- **Typo-tolerant fuzzy search** — new tier 4 search using edit distance (Levenshtein), allowing 1-2 typos depending on word length

### Fixed
- **Fuzzy search no longer matches random gibberish** — subsequence matching now requires 30% compactness density, preventing false positives from scattered character matches

---

## [1.8.3] - 2026-04-09

### Fixed
- **Sync no longer creates duplicate folders/clips** — when UUID doesn't match locally (e.g. after DB repair), sync now matches clips by `content_hash` and folders by name, adopting the remote UUID instead of creating duplicates
- **Auto-cleanup "(synced)" folders** — sync automatically skips artifact folders with "(synced)" suffix from cloud, and merges any existing local "(synced)" folders into their originals
- **Deduplicated rescan commands** — `rescan_sensitive` and `rescan_subtypes` commands now delegate to the batched database methods instead of using N+1 individual queries

---

## [1.8.1] - 2026-04-08

### Fixed
- **Fix unwrap panics** — replaced 5 `.unwrap()` calls in `lib.rs` and `data.rs` with proper error handling to prevent crashes when window or data directory is unavailable
- **Batch database rescans** — `rescan_sensitive()` and `rescan_subtypes()` now use batched SQL updates (500 per batch) instead of individual UPDATE per row, significantly faster on large clip histories
- **Reduce string allocations** — replaced ~25 occurrences of `from_utf8_lossy().to_string()` with `.into_owned()` across the Rust backend, and search words use `&str` slices instead of owned `String`

### Added
- **React Error Boundary** — wraps the entire app to catch render crashes and show a recovery UI instead of a blank screen
- **Image loading queue** — fallback image loads (when asset protocol fails) now go through a concurrency-limited queue (max 3 simultaneous) to prevent UI stutter
- **WAL checkpoint on shutdown** — ensures all data is flushed from WAL to main DB file before closing, preventing data loss on unexpected exits
- **Periodic WAL checkpoint** — background task checkpoints WAL every 5 minutes, reducing the window of data loss if the process is force-killed
- **Startup integrity check** — checks database integrity on launch; if corrupt, automatically backs up the bad file and creates a fresh DB with the original schema

### Improved
- **Smarter preview cache invalidation** — folder preview cache now only invalidates the affected folder's entry instead of clearing the entire cache on every clip change

---

## [1.7.6] - 2026-04-06

### Fixed
- **Image paste race condition** — images now written to clipboard by backend (via `write_image` plugin API) instead of frontend `navigator.clipboard.write`, fixing bug where pasting an image would paste old/wrong clipboard content due to timing mismatch between clipboard write and Shift+Insert
- **Image copy uses backend** — `copy_clip` for images also moved to backend, eliminating stale state and silent failure issues

---

## [1.7.5] - 2026-04-05

### Added
- **Image thumbnails** — auto-generate 280px JPEG thumbnails for clipboard images; list view loads lightweight thumbnails instead of full PNGs, reducing memory and load time
- **Smart Collections** — auto-categorize text clips by subtype: URL, email, color, path, phone, JSON, code; each gets a dedicated badge and custom card renderer
- **Subtype filter** — compact inline filter buttons in search bar to quickly filter by content type or smart subtype
- **`rescan_subtypes` command** — re-scan all existing text clips with latest subtype detection rules (runs on startup)

### Fixed
- **URL false-positive sensitive blur** — URLs no longer trigger the password-like string detection (mixed char classes in domains/paths)

### Changed
- **Unified clip filter** — merged content type and subtype filters into one compact row (9 icons), removed duplicates (url, file)
- **Thumbnail cleanup** — delete, bulk delete, max_items enforcement, auto-delete, and orphan cleanup all handle thumbnail files alongside originals

---

## [1.7.4] - 2026-04-04

### Added
- **Password detection** — auto-detect and blur random password-like strings (8-64 chars, 3+ character classes: upper/lower/digit/special)
- **Sensitive rescan on startup** — re-evaluates all existing clips with latest detection rules
- **CLI automation hooks** — `--list`, `--search`, `--get`, `--stats`, `--count`, `--clear`, `--help-cli`
- **`rescan_sensitive` command** — manually trigger re-scan from frontend or CLI

### Fixed
- **Sensitive detection on re-copy** — dedup now re-checks `is_sensitive` when bumping existing clips
- **Image paste duplicate** — reorder clipboard write to prevent listener capturing self-paste as new clip
- **Path traversal hardened** — `canonicalize()` verification + 500MB file size limit in import

---

## [1.7.3] - 2026-04-04

### Fixed
- **Incognito race condition** — check moved after acquiring clipboard lock to prevent bypass during debounce
- **Keyboard shortcuts in search** — Arrow Up/Down and Enter now work while typing in search input; E, P, Ctrl+Delete blocked while typing
- **Stale closures in modals** — EditClipModal and NoteModal use refs to prevent capturing old handler references
- **Settings rollback on failure** — UI reverts to previous settings if backend save fails
- **Context menu crash** — null guard prevents crash when clip/folder is deleted between menu open and render
- **Image paste error handling** — shows toast error and aborts if image fetch fails instead of silently continuing

### Changed
- **window-vibrancy pinned to commit** — reproducible builds (was `branch = "dev"`)
- **Clipboard debounce configurable** — reads `debounce_ms` from settings cache (default 150ms)
- **Dark mode folder contrast** — inactive folder buttons more visible (opacity /10 → /20)
- **Filter buttons visibility** — content type filter icons no longer half-transparent
- **Modal backdrop consistency** — all modals standardized to bg-black/50
- **Pin button always visible** — dimmed (40%) when unpinned instead of hidden
- **Sensitive blur softened** — reduced from 5px to 3px with faster transition
- **Batch action bar animation** — slide-in from bottom on multi-select
- **Disabled buttons** — cursor-not-allowed on disabled state

### Removed
- **14 clippy warnings** — type alias, strip_prefix, is_multiple_of, collapsed else-if, is_some_and, Copy trait, wildcard pattern
- **Dead code** — removed unused `isSearchPending` prop and skeleton UI from ClipList
- **ARIA improvements** — added role/aria-label/aria-pressed to filter buttons, incognito toggle, action buttons

---

## [1.7.0] - 2026-04-04

### Added
- **Incognito mode** — pause clipboard recording with EyeOff toggle button in control bar
- **Sensitive content detection** — auto-detect API keys (AWS, GitHub, Stripe, Slack), private keys, JWTs, credit cards (Luhn check); shield icon + blur on card content
- **Fuzzy search** — subsequence matching fallback ("apikey" matches "api_key", "API_KEY")
- **Search relevance ranking** — exact phrase > all words > note match > fuzzy; relevance before folder priority
- **Frequently Pasted** smart folder — virtual "Frequent" tab showing clips with paste_count >= 5
- **Acrylic & Blur window effects** — new options in Settings > Window Effect dropdown
- **Batch IPC** — `get_initial_state` command fetches clips + folders + count in 1 call via `tokio::join!()`
- **Asset protocol for images** — images served via `asset://` URL instead of base64 encoding (75% less IPC payload)
- **ARIA accessibility** — role=listbox, role=option, aria-selected, aria-label on clip list and cards; role=tablist on folder tabs
- **88 Rust tests** (was 65) — sensitive detection, fuzzy search, LRU cache, schema v5
- **125 frontend tests** — sensitive UI, incognito button, ARIA, subtype badges, multi-select, empty state
- **Architecture docs** — comprehensive `docs/architecture.md` with data flow diagrams

### Changed
- **SEARCH_CACHE** Vec → HashMap for O(1) remove/update, capped at 50,000 entries
- **ICON_CACHE** HashMap → LRU(100) to prevent unbounded memory growth
- **Per-connection PRAGMAs** via `after_connect` hook — all 5 pool connections get cache_size, mmap, foreign_keys
- **Covering index** `idx_clips_folder_created` for faster folder listing (DB migration v5)
- **Folder tabs** rounded-lg instead of rounded-full for softer corners
- **Folder drag-drop** improved with left/right drop indicator line
- **Card depth effects** — inner glow, deeper shadows, blue-tinted dark theme, header separator
- **App.tsx refactored** — extracted useContextMenu, useFolderModal, useBatchActions hooks (~120 lines removed)
- **ClipList** stable callback refs via useRef — prevents memo-defeating inline closures
- **Removed macOS build** from CI release workflow

### Fixed
- **SEARCH_CACHE not invalidated** after enforce_max_items, clear_all_clips, delete_folder, remove_duplicate_clips — stale search results
- **import_data overwrites live DB** — now extracts to temp dir first, backs up current DB
- **register_global_shortcut loses toggle** — hotkey now properly toggles show/hide after changing in Settings
- **enforce_auto_delete race condition** — wrapped in transaction
- **reorder_folders not atomic** — wrapped in transaction
- **Autostart placeholder flags** removed (--flag1, --flag2)
- **PRAGMA optimize** added after bulk clear/dedup operations

### Removed
- Dead `react-window` + `@types/react-window` dependencies
- Stale `SearchBar.test.tsx` for non-existent component
- macOS build targets from CI workflow

## [1.6.5] - 2026-04-02

### Added
- **Batch operations** — Ctrl+Click / Shift+Click to multi-select clips; bulk paste (joined by newline), delete, and move to folder via floating action bar
- **Search notes** — search now matches against clip notes in addition to content; note results sorted after content matches
- **Folder sort by note** — clips in folders sorted: pinned → has note (A→Z) → no note (newest first)
- **Mica effect on settings window** — settings window now applies same window effect as main window
- **Delete confirmation** — toast-based confirm before deleting clips (single and bulk)
- **Zip export verification** — exported backup zip is verified for integrity after creation

### Changed
- **Esc key chain** — progressive dismiss: multi-select → search → selected clip → folder → hide window
- **Folder persistence** — selected folder is kept when window reopens (not reset to All)
- **Drag-to-external hides window** — dragging a clip to an external app now auto-hides the window
- **Mouse wheel scroll** — improved scroll speed for mouse wheel (2.5x multiplier) while keeping trackpad smooth
- **Focus debounce** — window focus reload debounced 150ms to prevent query spam on rapid Alt+Tab
- **Log levels** — hot-path logs (get_clips, clipboard processing) reduced from info to debug/trace
- **EditClipModal** — UI text translated from Vietnamese to English
- **Clipboard write helper** — extracted shared clipboard write logic (stop listener → retry 5x → restart) into `clipboard_write_text()` helper

### Fixed
- **delete_clip orphan images** — always clean up image files on delete (previously only cleaned when `hard_delete=true`)
- **enforce_max_items race** — image filename query now matches exact rows being deleted via subquery
- **SEARCH_CACHE sync** — cache updated on move_to_folder, bulk_move, and update_note (previously stale until restart)
- **Import cache rebuild** — SEARCH_CACHE and SETTINGS_CACHE rebuilt after importing backup
- **pick_file path sanitization** — output path validated against path traversal and control characters

### Removed
- **Ctrl+1..9 quick-paste** — removed index badges from clip cards (feature was not functional)

---

## [1.6.1] - 2026-04-01

### Fixed
- **CI build** — bumped version after re-tag caused duplicate release error
- **README** — added Mermaid architecture diagrams (system overview, data flow, paste flow)

---

## [1.6.0] - 2026-04-01

### Added
- **Quick-paste shortcuts (Ctrl+1..9)** — press Ctrl+1 to paste the first clip, Ctrl+2 for second, up to Ctrl+9; index badge shown on card footer
- **Search highlight** — matching keywords highlighted in yellow within clip card text
- **Relative timestamp** — card footer now shows "2m", "1h", "3d" etc. alongside character count
- **`copy_clip` command** — new backend command that copies to clipboard without hiding the window or simulating paste (fixes Copy button behavior)

### Changed
- **Backend modularized** — monolithic `commands.rs` (1504 lines) split into 7 domain modules: `clips.rs`, `folders.rs`, `settings.rs`, `data.rs`, `window.rs`, `helpers.rs`, `mod.rs`
- **Frontend refactored** — App.tsx logic extracted into 4 custom hooks: `useClipActions`, `useFolderActions`, `useDragDrop`, `useFolderPreview`; SettingsPanel split into `GeneralTab`, `FoldersTab`, `DashboardTab`
- **47 Rust tests** — comprehensive test suite for subtype detection, UTF-8 truncation, hash, search cache, settings cache, and integration tests

### Fixed
- **Copy button hid window** — `handleCopy` was calling `paste_clip` (which hides window + simulates paste); now uses dedicated `copy_clip`
- **Image size display** — card footer now reads actual `size_bytes` from metadata instead of estimating from base64 string length

### Performance
- **Async image I/O** — `clip_to_item_async` uses `tokio::fs::read` instead of blocking `std::fs::read`, preventing Tokio runtime stalls
- **loadClips stability** — removed `clips.length` from useCallback dependency array, eliminating cascading re-renders
- **Stagger animation** — uses viewport-relative index instead of absolute index, preventing 1.5s+ invisible delays on scrolled items
- **Wheel scroll** — changed from `behavior: 'smooth'` to `'auto'`, eliminating scroll accumulation lag
- **Context menu** — single `clips.find()` call instead of 3-4 repeated lookups per render
- **ControlBar** — color arrays moved to module-level constants; folder categories wrapped in `useMemo`
- **Dynamic skeleton count** — skeleton cards computed from viewport width instead of hardcoded 8

---

## [1.5.0] - 2026-04-01

### Added
- **Dashboard** — stats overview (total clips, today, images, folders), activity chart (7 days), top source apps, most pasted clips, storage info
- **History Timeline** — browse clips by date with calendar picker, search within any day, click chart bars to navigate
- **Paste as plain text** — right-click context menu option to strip formatting
- **Export / Import backup** — zip archive (clipboard.db + images/), accessible from Settings
- **Search relevance ranking** — exact substring matches appear before partial word matches
- **Clip saved toast** — visual confirmation when clipboard captures a new clip
- **Schema version tracking** — migration table prevents duplicate ALTER TABLE runs

### Changed
- **Hard delete** — clips are permanently removed (no soft delete bloat)
- **SQLite DELETE journal mode** — reverted from WAL; data writes directly to .db file, no .db-wal/.db-shm files
- **Timezone-aware queries** — dashboard and history use `localtime` for correct date grouping
- **README rewritten** — architecture diagram, data flow, design decisions, updated feature list

### Fixed
- **Multi-word search** — "docker compose" now finds clips containing both words (AND logic)
- **Date query timezone** — clips grouped by local date, not UTC
- **Export/Import dialog** — PowerShell STA mode + spawn_blocking prevents UI freeze
- **Image card visibility** — dark background + border for screenshots on dark theme
- **Dashboard SQL** — fixed broken WHERE clauses from batch replace

### Performance
- **Composite index** — `idx_clips_deleted_created` on (is_deleted, created_at) for faster queries
- **Startup cleanup** — purge legacy soft-deleted rows, enforce max_items, clean orphan images

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
[Unreleased]: https://github.com/Phieu-Tran/ClipPaste/compare/v1.6.1...HEAD
[1.6.1]: https://github.com/Phieu-Tran/ClipPaste/compare/v1.6.0...v1.6.1
[1.6.0]: https://github.com/Phieu-Tran/ClipPaste/compare/v1.5.0...v1.6.0
[1.5.0]: https://github.com/Phieu-Tran/ClipPaste/compare/v1.4.8...v1.5.0
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
