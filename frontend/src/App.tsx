import { useEffect, useState, useCallback, useRef, useMemo } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';
import { getCurrentWindow } from '@tauri-apps/api/window';
import { WebviewWindow } from '@tauri-apps/api/webviewWindow';
import { ClipboardItem as AppClipboardItem, Settings, ClipType } from './types';
import { ClipList } from './components/ClipList';
import { ControlBar } from './components/ControlBar';
import { ContextMenu } from './components/ContextMenu';
import { FolderModal } from './components/FolderModal';
import { EditClipModal } from './components/EditClipModal';
import { NoteModal } from './components/NoteModal';
import { useKeyboard } from './hooks/useKeyboard';
import { useTheme } from './hooks/useTheme';
import { useClipActions } from './hooks/useClipActions';
import { useFolderActions } from './hooks/useFolderActions';
import { useDragDrop } from './hooks/useDragDrop';
import { useFolderPreview } from './hooks/useFolderPreview';
import { Toaster, toast } from 'sonner';
import { LAYOUT } from './constants';

function App() {
  const [clips, setClips] = useState<AppClipboardItem[]>([]);
  const [selectedFolder, setSelectedFolder] = useState<string | null>(null);
  const [searchQuery, setSearchQuery] = useState('');
  const [showSearch, setShowSearch] = useState(false);
  const [contentTypeFilter, setContentTypeFilter] = useState<ClipType | null>(null);
  const [selectedClipId, setSelectedClipId] = useState<string | null>(null);
  const [selectedClipIds, setSelectedClipIds] = useState<Set<string>>(new Set());
  const [isLoading, setIsLoading] = useState(true);
  const [hasMore, setHasMore] = useState(true);
  const [theme, setTheme] = useState('system');

  // Add Folder Modal State
  const [showAddFolderModal, setShowAddFolderModal] = useState(false);
  const [newFolderName, setNewFolderName] = useState('');

  const searchInputRef = useRef<HTMLInputElement>(null);
  const [windowFocusCount, setWindowFocusCount] = useState(0);

  const effectiveTheme = useTheme(theme);

  const appWindow = getCurrentWindow();
  const selectedFolderRef = useRef(selectedFolder);
  selectedFolderRef.current = selectedFolder;

  // Edit clip before paste
  const [editingClip, setEditingClip] = useState<AppClipboardItem | null>(null);

  // Note modal state
  const [noteModalClipId, setNoteModalClipId] = useState<string | null>(null);
  const [noteModalInitial, setNoteModalInitial] = useState('');

  // --- Folder Actions Hook ---
  const {
    folders,
    totalClipCount,
    loadFolders,
    refreshTotalCount,
    handleCreateFolder,
    handleDeleteFolder,
    handleReorderFolders,
    handleMoveClip,
    debouncedFolderRefresh,
  } = useFolderActions({
    selectedFolder,
    setSelectedFolder,
    setClips,
  });

  // --- Clip Actions Hook ---
  const {
    loadClips,
    autoSelectFirstOnNextLoadRef,
    handleDelete,
    handlePaste,
    handleCopy,
    handleTogglePin,
    handleEditBeforePaste,
    handlePasteEdited,
    handlePastePlainText,
    handleEditNote,
    handleSaveNote,
  } = useClipActions({
    clips,
    setClips,
    setIsLoading,
    setHasMore,
    setSelectedClipId,
    setEditingClip,
    setNoteModalClipId,
    setNoteModalInitial,
    loadFolders,
    refreshTotalCount,
    refreshCurrentFolder: () => refreshCurrentFolder(),
  });

  // --- Drag & Drop Hook ---
  const {
    draggingClipId,
    dragTargetFolderId,
    handleDragHover,
    handleDragLeave,
    handleNativeDragStart,
    handleNativeDragEnd,
  } = useDragDrop({ handleMoveClip });

  // --- Folder Preview Hook ---
  const {
    previewFolder,
    setPreviewFolder,
    previewClips,
    isPreviewLoading,
    isPreviewing,
    handleFolderHover,
    handleFolderHoverEnd,
    handlePreviewListEnter,
    handlePreviewListLeave,
  } = useFolderPreview({ clips, folders });

  const refreshCurrentFolder = useCallback(() => {
    loadClips(selectedFolderRef.current, false, searchQuery);
  }, [loadClips, searchQuery]);

  // Stable ref so the clipboard listener never re-subscribes
  const refreshCurrentFolderRef = useRef(refreshCurrentFolder);
  refreshCurrentFolderRef.current = refreshCurrentFolder;

  // Stable ref for loadClips — used in focus handler to bypass stale closures
  const loadClipsRef = useRef(loadClips);
  loadClipsRef.current = loadClips;
  const debouncedFolderRefreshRef = useRef(debouncedFolderRefresh);
  debouncedFolderRefreshRef.current = debouncedFolderRefresh;

  const [searchInput, setSearchInput] = useState('');
  const searchTimerRef = useRef<ReturnType<typeof setTimeout> | null>(null);

  const handleSearch = useCallback((query: string) => {
    setSearchInput(query);
    setPreviewFolder(undefined);
    if (searchTimerRef.current) clearTimeout(searchTimerRef.current);
    searchTimerRef.current = setTimeout(() => {
      setSearchQuery(query);
    }, 100);
  }, [setPreviewFolder]);

  // --- Effects ---

  useEffect(() => {
    invoke<Settings>('get_settings')
      .then((s) => {
        setTheme(s.theme);
      })
      .catch(console.error);

    const unlisten = listen<Settings>('settings-changed', (event) => {
      setTheme(event.payload.theme);
    });

    return () => {
      unlisten.then((f) => f());
    };
  }, []);

  // Auto-show search bar when window opens
  useEffect(() => {
    setShowSearch(true);
    setTimeout(() => {
      searchInputRef.current?.focus();
    }, 100);
  }, []);

  // Reset selection, clear search, reload clips, and scroll to top every time the window is shown/focused
  // Debounced to avoid spam queries on rapid Alt+Tab toggles
  const focusTimerRef = useRef<ReturnType<typeof setTimeout> | null>(null);
  useEffect(() => {
    const unlisten = appWindow.listen('tauri://focus', () => {
      if (focusTimerRef.current) clearTimeout(focusTimerRef.current);
      focusTimerRef.current = setTimeout(() => {
        setSelectedClipId(null);
        setSelectedClipIds(new Set());
        // Keep selectedFolder — user stays in their folder across window toggles
        setSearchQuery('');
        setSearchInput('');
        setContentTypeFilter(null);
        setPreviewFolder(undefined);
        autoSelectFirstOnNextLoadRef.current = true;
        setWindowFocusCount((c) => c + 1);
        loadClipsRef.current(selectedFolderRef.current, false, '');
      }, 150);
    });
    return () => {
      unlisten.then((f) => f());
    };
  // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  // Focus search input AFTER React has rendered the cleared state
  useEffect(() => {
    if (windowFocusCount > 0) {
      searchInputRef.current?.focus();
    }
  }, [windowFocusCount]);

  useEffect(() => {
    loadFolders();
    if (searchQuery.trim()) {
      loadClips(selectedFolder, false, searchQuery);
    } else {
      loadClips(selectedFolder);
    }
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [selectedFolder, searchQuery]);

  useEffect(() => {
    refreshTotalCount();
  }, [refreshTotalCount]);

  // Subscribe ONCE — uses refs so the callback is always fresh without re-subscribing
  useEffect(() => {
    const unlistenClipboard = listen<{ clip_type?: string }>('clipboard-change', (event) => {
      refreshCurrentFolderRef.current();
      debouncedFolderRefreshRef.current();
      const type = event.payload?.clip_type || 'text';
      toast.success(type === 'image' ? 'Image saved' : 'Clip saved', {
        duration: 1500,
        style: { fontSize: '12px', padding: '6px 12px' },
      });
    });

    return () => {
      unlistenClipboard.then((unlisten) => {
        if (typeof unlisten === 'function') unlisten();
      });
    };
  }, []);

  // --- Settings window ---
  const openSettings = useCallback(async () => {
    const existingWin = await WebviewWindow.getByLabel('settings');
    if (existingWin) {
      try {
        await invoke('focus_window', { label: 'settings' });
      } catch (e) {
        console.error('Failed to focus settings window:', e);
        await existingWin.unminimize();
        await existingWin.show();
        await existingWin.setFocus();
      }
      return;
    }

    const settingsWin = new WebviewWindow('settings', {
      url: 'index.html?window=settings',
      title: 'Settings',
      width: 800,
      height: 700,
      resizable: true,
      decorations: false,
      center: true,
    });

    settingsWin.once('tauri://created', function () {});
    settingsWin.once('tauri://error', function (e) {
      console.error('Error creating settings window', e);
    });
  }, []);

  // --- Keyboard shortcuts ---
  useKeyboard({
    onClose: () => {
      if (editingClip) return;
      // Esc priority: multi-select → search → selected clip → folder → hide window
      if (isMultiSelect) {
        setSelectedClipIds(new Set());
        setSelectedClipId(null);
      } else if (searchInput.trim()) {
        handleSearch('');
        searchInputRef.current?.focus();
      } else if (selectedClipId) {
        setSelectedClipId(null);
      } else if (selectedFolder) {
        setSelectedFolder(null);
      } else {
        appWindow.hide();
      }
    },
    onSearch: () => {
      if (editingClip) return;
      setShowSearch(true);
      setTimeout(() => {
        searchInputRef.current?.focus();
      }, 50);
    },
    onDelete: () => {
      if (editingClip) return;
      if (isMultiSelect) { handleBulkDelete(); }
      else { handleDelete(selectedClipId); }
    },
    onNavigateUp: () => {
      if (editingClip || isLoading) return;
      const currentIndex = clips.findIndex((c) => c.id === selectedClipId);
      if (currentIndex > 0) {
        setSelectedClipId(clips[currentIndex - 1].id);
      }
    },
    onNavigateDown: () => {
      if (editingClip || isLoading) return;
      const currentIndex = clips.findIndex((c) => c.id === selectedClipId);
      if (currentIndex === -1 && clips.length > 0) {
        setSelectedClipId(clips[0].id);
      } else if (currentIndex < clips.length - 1) {
        setSelectedClipId(clips[currentIndex + 1].id);
      }
    },
    onPaste: () => {
      if (editingClip) return;
      if (isMultiSelect) { handleBulkPaste(); }
      else if (selectedClipId) { handlePaste(selectedClipId); }
    },
    onEdit: () => {
      if (selectedClipId && !editingClip) {
        handleEditBeforePaste(selectedClipId);
      }
    },
    onPin: () => {
      if (selectedClipId && !editingClip && selectedFolder) {
        handleTogglePin(selectedClipId);
      }
    },
  });

  // --- Load more (infinite scroll) ---
  const loadMore = useCallback(() => {
    if (hasMore && !isLoading) {
      loadClips(selectedFolder, true, searchQuery);
    }
  }, [hasMore, isLoading, selectedFolder, loadClips, searchQuery]);

  // --- Context Menu ---
  const [contextMenu, setContextMenu] = useState<{
    type: 'card' | 'folder';
    x: number;
    y: number;
    itemId: string;
  } | null>(null);

  // New Folder Modal Rename Mode
  const [folderModalMode, setFolderModalMode] = useState<'create' | 'rename'>('create');
  const [editingFolderId, setEditingFolderId] = useState<string | null>(null);
  const [editingFolderColor, setEditingFolderColor] = useState<string | null>(null);
  const [editingFolderIcon, setEditingFolderIcon] = useState<string | null>(null);

  // Smart content type matching
  const matchesContentType = useCallback((clip: AppClipboardItem, filter: ClipType): boolean => {
    if (filter === 'image') return clip.clip_type === 'image';
    if (filter === 'url') {
      return clip.clip_type === 'text' && /^https?:\/\/\S+$/i.test(clip.content.trim());
    }
    if (filter === 'file') {
      return clip.clip_type === 'text' && /^[a-zA-Z]:\\/.test(clip.content.trim());
    }
    if (filter === 'html') return clip.clip_type === 'html';
    if (filter === 'rtf') return clip.clip_type === 'rtf';
    if (filter === 'text') {
      return clip.clip_type === 'text'
        && !/^https?:\/\/\S+$/i.test(clip.content.trim())
        && !/^[a-zA-Z]:\\/.test(clip.content.trim());
    }
    return clip.clip_type === filter;
  }, []);

  const filteredClips = useMemo(() => {
    if (!contentTypeFilter) return clips;
    return clips.filter((c) => matchesContentType(c, contentTypeFilter));
  }, [clips, contentTypeFilter, matchesContentType]);

  const filteredPreviewClips = useMemo(() => {
    if (!contentTypeFilter) return previewClips;
    return previewClips.filter((c) => matchesContentType(c, contentTypeFilter));
  }, [previewClips, contentTypeFilter, matchesContentType]);

  const folderMap = useMemo(() => {
    const map: Record<string, string> = {};
    for (const f of folders) { map[f.id] = f.name; }
    return map;
  }, [folders]);

  const handleContextMenu = useCallback(
    (e: React.MouseEvent, type: 'card' | 'folder', itemId: string) => {
      e.preventDefault();
      setContextMenu({ type, x: e.clientX, y: e.clientY, itemId });
    },
    []
  );

  const handleCloseContextMenu = useCallback(() => {
    setContextMenu(null);
  }, []);

  const handleCreateOrRenameFolder = async (name: string, color: string | null, icon: string | null) => {
    if (folderModalMode === 'create') {
      await handleCreateFolder(name, color, icon);
      toast.success(`Folder "${name}" created`);
      setShowAddFolderModal(false);
      setNewFolderName('');
    } else if (folderModalMode === 'rename' && editingFolderId) {
      try {
        await invoke('rename_folder', { id: editingFolderId, name, color, icon });
        await loadFolders();
        toast.success(`Renamed to "${name}"`);
        setShowAddFolderModal(false);
        setNewFolderName('');
      } catch (error) {
        console.error('Failed to rename folder:', error);
        toast.error('Failed to rename folder');
      }
    }
  };

  // --- Multi-select ---
  const handleSelectClip = useCallback((clipId: string, e?: React.MouseEvent) => {
    const displayedClips = isPreviewing ? filteredPreviewClips : filteredClips;

    if (e?.shiftKey && selectedClipId) {
      // Range select: from last selected to clicked
      const startIdx = displayedClips.findIndex(c => c.id === selectedClipId);
      const endIdx = displayedClips.findIndex(c => c.id === clipId);
      if (startIdx !== -1 && endIdx !== -1) {
        const [from, to] = startIdx < endIdx ? [startIdx, endIdx] : [endIdx, startIdx];
        const rangeIds = displayedClips.slice(from, to + 1).map(c => c.id);
        setSelectedClipIds(prev => {
          const next = new Set(prev);
          if (selectedClipId && !next.has(selectedClipId)) {
            next.add(selectedClipId);
          }
          rangeIds.forEach(id => next.add(id));
          return next;
        });
      }
    } else if (e?.ctrlKey || e?.metaKey) {
      // Toggle select — also include the currently selected clip if not yet in set
      setSelectedClipIds(prev => {
        const next = new Set(prev);
        if (selectedClipId && !next.has(selectedClipId)) {
          next.add(selectedClipId);
        }
        if (next.has(clipId)) {
          next.delete(clipId);
        } else {
          next.add(clipId);
        }
        return next;
      });
    } else {
      // Single select — clear multi-select
      setSelectedClipIds(new Set());
    }
    setSelectedClipId(clipId);
  }, [selectedClipId, isPreviewing, filteredPreviewClips, filteredClips]);

  const isMultiSelect = selectedClipIds.size > 1;

  const handleBulkDelete = useCallback(async () => {
    if (selectedClipIds.size === 0) return;
    const ids = Array.from(selectedClipIds);
    toast(`Delete ${ids.length} clips?`, {
      action: {
        label: 'Delete',
        onClick: async () => {
          try {
            const count = await invoke<number>('bulk_delete_clips', { ids });
            setClips(prev => prev.filter(c => !selectedClipIds.has(c.id)));
            setSelectedClipIds(new Set());
            setSelectedClipId(null);
            loadFolders();
            refreshTotalCount();
            toast.success(`Deleted ${count} clips`);
          } catch (error) {
            console.error('Bulk delete failed:', error);
            toast.error('Failed to delete clips');
          }
        },
      },
      cancel: { label: 'Cancel', onClick: () => {} },
      duration: 4000,
    });
  }, [selectedClipIds, setClips, loadFolders, refreshTotalCount]);

  const handleBulkMove = useCallback(async (folderId: string | null) => {
    if (selectedClipIds.size === 0) return;
    const ids = Array.from(selectedClipIds);
    try {
      await invoke('bulk_move_clips', { ids, folderId });
      if (selectedFolder && folderId !== selectedFolder) {
        setClips(prev => prev.filter(c => !selectedClipIds.has(c.id)));
      } else {
        setClips(prev => prev.map(c => selectedClipIds.has(c.id) ? { ...c, folder_id: folderId } : c));
      }
      setSelectedClipIds(new Set());
      setSelectedClipId(null);
      loadFolders();
      refreshTotalCount();
      toast.success(`Moved ${ids.length} clips`);
    } catch (error) {
      console.error('Bulk move failed:', error);
      toast.error('Failed to move clips');
    }
  }, [selectedClipIds, selectedFolder, setClips, loadFolders, refreshTotalCount]);

  const handleBulkPaste = useCallback(async () => {
    if (selectedClipIds.size === 0) return;
    const displayedClips = isPreviewing ? filteredPreviewClips : filteredClips;
    // Collect selected clips in display order, skip images
    const textsInOrder = displayedClips
      .filter(c => selectedClipIds.has(c.id) && c.clip_type !== 'image')
      .map(c => c.content);
    if (textsInOrder.length === 0) {
      toast.error('No text clips selected');
      return;
    }
    const combined = textsInOrder.join('\n');
    try {
      await invoke('paste_text', { content: combined });
      setSelectedClipIds(new Set());
      setSelectedClipId(null);
    } catch (error) {
      console.error('Bulk paste failed:', error);
      toast.error('Failed to paste');
    }
  }, [selectedClipIds, isPreviewing, filteredPreviewClips, filteredClips]);

  // --- Render ---
  return (
    <div className="relative h-screen w-full overflow-hidden" onDragOver={(e) => { if (draggingClipId) e.preventDefault(); }} onDragEnd={handleNativeDragEnd}>
      <div
        className="pointer-events-none absolute inset-0"
        style={{
          backgroundColor: 'transparent',
          backdropFilter: 'blur(2px)',
        }}
      />

      <div className="relative h-full w-full" style={{ padding: `${LAYOUT.WINDOW_PADDING}px` }}>
        <div className="flex h-full w-full flex-col overflow-hidden rounded-[12px] border border-border/10 bg-background/80 text-foreground shadow-[0_4px_32px_rgba(0,0,0,0.15)] dark:shadow-[0_4px_32px_rgba(0,0,0,0.5)]">
          {contextMenu && (() => {
            const ctxClip = contextMenu.type === 'card' ? clips.find((c) => c.id === contextMenu.itemId) : null;
            const ctxFolder = contextMenu.type === 'folder' ? folders.find((f) => f.id === contextMenu.itemId) : null;
            return (
              <ContextMenu
                x={contextMenu.x}
                y={contextMenu.y}
                onClose={handleCloseContextMenu}
                options={
                  contextMenu.type === 'card'
                    ? [
                        ...(selectedFolder ? [{
                          label: ctxClip?.is_pinned ? 'Unpin' : 'Pin',
                          onClick: () => handleTogglePin(contextMenu.itemId),
                        }] : []),
                        ...(ctxClip?.clip_type !== 'image'
                          ? [
                              { label: 'Paste as plain text', onClick: () => handlePastePlainText(contextMenu.itemId) },
                              { label: 'Edit before paste', onClick: () => handleEditBeforePaste(contextMenu.itemId) },
                            ]
                          : []),
                        {
                          label: ctxClip?.note ? 'Edit note' : 'Add note',
                          onClick: () => handleEditNote(contextMenu.itemId),
                        },
                        {
                          label: 'Delete',
                          danger: true,
                          onClick: () => handleDelete(contextMenu.itemId),
                        },
                      ]
                    : [
                        {
                          label: 'Edit folder',
                          onClick: () => {
                            setFolderModalMode('rename');
                            setEditingFolderId(contextMenu.itemId);
                            setNewFolderName(ctxFolder ? ctxFolder.name : '');
                            setEditingFolderColor(ctxFolder?.color ?? null);
                            setEditingFolderIcon(ctxFolder?.icon ?? null);
                            setShowAddFolderModal(true);
                          },
                        },
                        {
                          label: 'Delete',
                          danger: true,
                          onClick: () => {
                            if (window.confirm(`Delete folder "${ctxFolder?.name}"? Clips inside will be moved to All.`)) {
                              handleDeleteFolder(contextMenu.itemId);
                            }
                          },
                        },
                      ]
                }
              />
            );
          })()}

          <EditClipModal
            clip={editingClip}
            onPaste={handlePasteEdited}
            onClose={() => setEditingClip(null)}
          />

          <NoteModal
            isOpen={!!noteModalClipId}
            clipId={noteModalClipId}
            initialNote={noteModalInitial}
            onSave={handleSaveNote}
            onClose={() => setNoteModalClipId(null)}
          />

          <ControlBar
            ref={searchInputRef}
            folders={folders}
            selectedFolder={selectedFolder}
            onSelectFolder={setSelectedFolder}
            showSearch={showSearch}
            searchQuery={searchInput}
            onSearchChange={handleSearch}
            onSearchClick={() => {
              if (showSearch) {
                handleSearch('');
              }
              setShowSearch(!showSearch);
            }}
            onAddClick={() => {
              setFolderModalMode('create');
              setNewFolderName('');
              setShowAddFolderModal(true);
            }}
            onMoreClick={openSettings}
            isDragging={!!draggingClipId}
            dragTargetFolderId={dragTargetFolderId}
            onDragHover={handleDragHover}
            onDragLeave={handleDragLeave}
            totalClipCount={totalClipCount}
            onFolderContextMenu={(e, folderId) => {
              if (folderId) handleContextMenu(e, 'folder', folderId);
            }}
            onReorderFolders={handleReorderFolders}
            onFolderHover={handleFolderHover}
            onFolderHoverEnd={handleFolderHoverEnd}
            theme={effectiveTheme}
            contentTypeFilter={contentTypeFilter}
            onContentTypeFilterChange={setContentTypeFilter}
          />

          <main
            className="no-scrollbar relative flex-1"
            onMouseEnter={isPreviewing ? handlePreviewListEnter : undefined}
            onMouseLeave={isPreviewing ? handlePreviewListLeave : undefined}
          >
            <ClipList
              clips={isPreviewing ? filteredPreviewClips : filteredClips}
              isLoading={isPreviewing ? isPreviewLoading : isLoading}
              hasMore={isPreviewing ? false : hasMore}
              selectedClipId={selectedClipId}
              selectedClipIds={selectedClipIds}
              onSelectClip={handleSelectClip}
              onPaste={handlePaste}
              onCopy={handleCopy}
              onPin={handleTogglePin}
              showPin={!!(isPreviewing ? previewFolder : selectedFolder)}
              onLoadMore={isPreviewing ? () => {} : loadMore}
              resetScrollKey={isPreviewing ? undefined : windowFocusCount}
              onNativeDragStart={handleNativeDragStart}
              onCardContextMenu={(e, clipId) => handleContextMenu(e, 'card', clipId)}
              isPreviewing={isPreviewing}
              isSearching={!!searchQuery.trim()}
              folderMap={folderMap}
              selectedFolder={selectedFolder}
              searchQuery={searchQuery}
            />
          </main>

          {/* Batch action bar — floating overlay */}
          {isMultiSelect && (
            <div className="pointer-events-none absolute bottom-6 left-0 right-0 z-40 flex justify-center">
              <div className="pointer-events-auto flex items-center gap-2 rounded-full border border-border/50 bg-background/95 px-4 py-2 shadow-xl backdrop-blur-md">
                <span className="text-xs font-semibold text-primary">{selectedClipIds.size} selected</span>
                <div className="h-3.5 w-px bg-border/50" />
                <button
                  onClick={handleBulkPaste}
                  className="rounded-full bg-primary/15 px-3 py-1 text-xs font-medium text-primary transition-colors hover:bg-primary/25"
                >
                  Paste
                </button>
                <button
                  onClick={handleBulkDelete}
                  className="rounded-full bg-destructive/15 px-3 py-1 text-xs font-medium text-destructive transition-colors hover:bg-destructive/25"
                >
                  Delete
                </button>
                {folders.length > 0 && (
                  <select
                    defaultValue=""
                    onChange={(e) => {
                      const val = e.target.value;
                      if (val === '__none__') handleBulkMove(null);
                      else if (val) handleBulkMove(val);
                      e.target.value = '';
                    }}
                    className="rounded-full border border-border/50 bg-card px-2.5 py-1 text-xs text-foreground"
                  >
                    <option value="" disabled>Move to...</option>
                    <option value="__none__">All (remove from folder)</option>
                    {folders.map(f => (
                      <option key={f.id} value={f.id}>{f.name}</option>
                    ))}
                  </select>
                )}
                <div className="h-3.5 w-px bg-border/50" />
                <button
                  onClick={() => { setSelectedClipIds(new Set()); setSelectedClipId(null); }}
                  className="rounded-full px-2 py-1 text-xs text-muted-foreground transition-colors hover:bg-accent hover:text-foreground"
                >
                  Cancel
                </button>
              </div>
            </div>
          )}

          <FolderModal
            isOpen={showAddFolderModal}
            mode={folderModalMode}
            initialName={newFolderName}
            initialColor={folderModalMode === 'rename' ? editingFolderColor : null}
            initialIcon={folderModalMode === 'rename' ? editingFolderIcon : null}
            onClose={() => {
              setShowAddFolderModal(false);
              setNewFolderName('');
            }}
            onSubmit={handleCreateOrRenameFolder}
          />
          <Toaster richColors position="bottom-center" theme={effectiveTheme} />
        </div>
      </div>
    </div>
  );
}

export default App;
