import { useEffect, useState, useCallback, useRef, useMemo } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { getCurrentWindow } from '@tauri-apps/api/window';
import { WebviewWindow } from '@tauri-apps/api/webviewWindow';
import { ClipboardItem as AppClipboardItem } from './types';
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
import { useContextMenu } from './hooks/useContextMenu';
import { useFolderModal } from './hooks/useFolderModal';
import { useBatchActions } from './hooks/useBatchActions';
import { useWindowLifecycle } from './hooks/useWindowLifecycle';
import { useSearch } from './hooks/useSearch';
import { useMultiSelect } from './hooks/useMultiSelect';
import { useScratchpad } from './hooks/useScratchpad';
import { Toaster, toast } from 'sonner';
import { LAYOUT } from './constants';

function App() {
  const [clips, setClips] = useState<AppClipboardItem[]>([]);
  const [selectedFolder, setSelectedFolder] = useState<string | null>(null);
  const [isLoading, setIsLoading] = useState(true);
  const [hasMore, setHasMore] = useState(true);
  const [theme, setTheme] = useState('system');

  const searchInputRef = useRef<HTMLInputElement>(null);

  const effectiveTheme = useTheme(theme);

  const appWindow = getCurrentWindow();
  const selectedFolderRef = useRef(selectedFolder);
  selectedFolderRef.current = selectedFolder;

  // Edit clip before paste
  const [editingClip, setEditingClip] = useState<AppClipboardItem | null>(null);

  // Note modal state
  const [noteModalClipId, setNoteModalClipId] = useState<string | null>(null);
  const [noteModalInitial, setNoteModalInitial] = useState('');

  // Incognito mode
  const [isIncognito, setIsIncognito] = useState(false);
  useEffect(() => {
    invoke<boolean>('get_incognito_status').then(setIsIncognito).catch(console.error);
  }, []);
  const toggleIncognito = useCallback(async () => {
    try {
      const newVal = await invoke<boolean>('toggle_incognito');
      setIsIncognito(newVal);
      toast.success(newVal ? 'Incognito mode ON — clipboard not recorded' : 'Incognito mode OFF');
    } catch (e) {
      console.error('Failed to toggle incognito:', e);
    }
  }, []);

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
    setSelectedClipId: (v) => setSelectedClipId(v),
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

  // --- Search Hook ---
  const {
    searchInput,
    searchQuery,
    showSearch,
    clipFilter,
    filteredClips,
    filteredPreviewClips,
    handleSearch,
    setShowSearch,
    setClipFilter,
  } = useSearch({ clips, previewClips, setPreviewFolder });

  // --- Multi-Select Hook ---
  const displayedClips = isPreviewing ? filteredPreviewClips : filteredClips;
  const {
    selectedClipId,
    selectedClipIds,
    isMultiSelect,
    setSelectedClipId,
    setSelectedClipIds,
    handleSelectClip,
  } = useMultiSelect({ displayedClips });

  const refreshCurrentFolder = useCallback(() => {
    loadClips(selectedFolderRef.current, false, searchQuery);
  }, [loadClips, searchQuery]);

  // Stable refs so event listeners never re-subscribe
  const loadClipsRef = useRef(loadClips);
  loadClipsRef.current = loadClips;
  const debouncedFolderRefreshRef = useRef(debouncedFolderRefresh);
  debouncedFolderRefreshRef.current = debouncedFolderRefresh;

  // --- Window Lifecycle Hook ---
  const { windowFocusCount, refreshCurrentFolderRef } = useWindowLifecycle({
    searchInputRef,
    selectedFolderRef,
    loadClipsRef,
    debouncedFolderRefreshRef,
    setClips,
    setHasMore,
    setIsLoading,
    setSelectedClipId,
    setSelectedClipIds,
    setPreviewFolder,
    setTheme,
  });

  // Keep refreshCurrentFolderRef in sync for clipboard-change listener
  refreshCurrentFolderRef.current = refreshCurrentFolder;

  // --- Effects ---

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
      const currentIndex = displayedClips.findIndex((c) => c.id === selectedClipId);
      if (currentIndex > 0) {
        setSelectedClipId(displayedClips[currentIndex - 1].id);
      }
    },
    onNavigateDown: () => {
      if (editingClip || isLoading) return;
      const currentIndex = displayedClips.findIndex((c) => c.id === selectedClipId);
      if (currentIndex === -1 && displayedClips.length > 0) {
        setSelectedClipId(displayedClips[0].id);
      } else if (currentIndex < displayedClips.length - 1) {
        setSelectedClipId(displayedClips[currentIndex + 1].id);
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

  // --- Context Menu (extracted hook) ---
  const { contextMenu, handleContextMenu, handleCloseContextMenu } = useContextMenu();

  // --- Folder Modal (extracted hook) ---
  const {
    showAddFolderModal, newFolderName, folderModalMode,
    editingFolderId, editingFolderColor, editingFolderIcon,
    openCreateModal, openRenameModal, closeModal: closeFolderModal,
  } = useFolderModal();

  const folderMap = useMemo(() => {
    const map: Record<string, string> = {};
    for (const f of folders) { map[f.id] = f.name; }
    return map;
  }, [folders]);

  const handleCreateOrRenameFolder = async (name: string, color: string | null, icon: string | null) => {
    if (folderModalMode === 'create') {
      await handleCreateFolder(name, color, icon);
      toast.success(`Folder "${name}" created`);
      closeFolderModal();
    } else if (folderModalMode === 'rename' && editingFolderId) {
      try {
        await invoke('rename_folder', { id: editingFolderId, name, color, icon });
        await loadFolders();
        toast.success(`Renamed to "${name}"`);
        closeFolderModal();
      } catch (error) {
        console.error('Failed to rename folder:', error);
        toast.error('Failed to rename folder');
      }
    }
  };

  // --- Scratchpad (auto-starts as separate window on right edge) ---
  const { toggle: toggleScratchpad } = useScratchpad();

  // --- Batch Actions (extracted hook) ---
  const { handleBulkDelete, handleBulkMove, handleBulkPaste } = useBatchActions({
    selectedClipIds, setSelectedClipIds, setSelectedClipId, setClips,
    selectedFolder, loadFolders, refreshTotalCount,
    isPreviewing, filteredPreviewClips: filteredPreviewClips, filteredClips,
  });

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
            // Guard: if clip/folder was deleted between context menu open and render, close menu
            if (contextMenu.type === 'card' && !ctxClip) return null;
            if (contextMenu.type === 'folder' && !ctxFolder) return null;
            return (
              <ContextMenu
                x={contextMenu.x}
                y={contextMenu.y}
                onClose={handleCloseContextMenu}
                options={
                  contextMenu.type === 'card' && ctxClip
                    ? [
                        ...(selectedFolder ? [{
                          label: ctxClip.is_pinned ? 'Unpin' : 'Pin',
                          onClick: () => handleTogglePin(contextMenu.itemId),
                        }] : []),
                        ...(ctxClip.clip_type !== 'image'
                          ? [
                              { label: 'Paste as plain text', onClick: () => handlePastePlainText(contextMenu.itemId) },
                              { label: 'Edit before paste', onClick: () => handleEditBeforePaste(contextMenu.itemId) },
                            ]
                          : []),
                        {
                          label: ctxClip.note ? 'Edit note' : 'Add note',
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
                            openRenameModal(
                              contextMenu.itemId,
                              ctxFolder ? ctxFolder.name : '',
                              ctxFolder?.color ?? null,
                              ctxFolder?.icon ?? null,
                            );
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
            onAddClick={openCreateModal}
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
            clipFilter={clipFilter}
            onClipFilterChange={setClipFilter}
            isIncognito={isIncognito}
            onToggleIncognito={toggleIncognito}
            onToggleScratchpad={toggleScratchpad}
          />

          <main
            className="no-scrollbar relative flex-1 bg-gradient-to-b from-transparent via-black/[0.02] to-black/[0.05] dark:via-black/[0.08] dark:to-black/[0.15]"
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
            <div className="animate-in fade-in slide-in-from-bottom-4 pointer-events-none absolute bottom-6 left-0 right-0 z-40 flex justify-center duration-200">
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
            onClose={closeFolderModal}
            onSubmit={handleCreateOrRenameFolder}
          />
          <Toaster richColors position="bottom-center" theme={effectiveTheme} />
        </div>
      </div>
    </div>
  );
}

export default App;
