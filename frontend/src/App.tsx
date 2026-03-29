import { useEffect, useState, useCallback, useRef } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';
import { getCurrentWindow } from '@tauri-apps/api/window';
import { WebviewWindow } from '@tauri-apps/api/webviewWindow';
import { ClipboardItem as AppClipboardItem, FolderItem, Settings } from './types';
import { ClipList } from './components/ClipList';
import { ControlBar } from './components/ControlBar';
import { DragPreview } from './components/DragPreview';
import { ContextMenu } from './components/ContextMenu';
import { FolderModal } from './components/FolderModal';
import { EditClipModal } from './components/EditClipModal';
import { useKeyboard } from './hooks/useKeyboard';
import { useTheme } from './hooks/useTheme';
import { Toaster, toast } from 'sonner';
import { LAYOUT } from './constants';

const base64ToBlob = (base64: string, mimeType: string = 'image/png'): Blob => {
  const byteCharacters = atob(base64);
  const byteNumbers = new Array(byteCharacters.length);
  for (let i = 0; i < byteCharacters.length; i++) {
    byteNumbers[i] = byteCharacters.charCodeAt(i);
  }
  const byteArray = new Uint8Array(byteNumbers);
  return new Blob([byteArray], { type: mimeType });
};

function App() {
  const [clips, setClips] = useState<AppClipboardItem[]>([]);
  const [folders, setFolders] = useState<FolderItem[]>([]);
  const [selectedFolder, setSelectedFolder] = useState<string | null>(null);
  const [searchQuery, setSearchQuery] = useState('');
  const [showSearch, setShowSearch] = useState(false);
  const [selectedClipId, setSelectedClipId] = useState<string | null>(null);
  const [isLoading, setIsLoading] = useState(true);
  const [hasMore, setHasMore] = useState(true);
  const [theme, setTheme] = useState('system');

  // Simulated Drag State
  const [draggingClipId, setDraggingClipId] = useState<string | null>(null);
  const [dragPosition, setDragPosition] = useState({ x: 0, y: 0 });
  const [dragTargetFolderId, setDragTargetFolderId] = useState<string | null>(null);

  // Add Folder Modal State
  const [showAddFolderModal, setShowAddFolderModal] = useState(false);
  const [newFolderName, setNewFolderName] = useState('');

  // Using refs for event handlers to access latest state without re-attaching listeners
  const dragStateRef = useRef({
    isDragging: false,
    clipId: null as string | null,
    targetFolderId: null as string | null,
    pendingDrag: null as { clipId: string; startX: number; startY: number } | null,
  });

  const searchInputRef = useRef<HTMLInputElement>(null);
  const isDeletingRef = useRef(false);
  const clipsRef = useRef(clips);
  clipsRef.current = clips;
  const [windowFocusCount, setWindowFocusCount] = useState(0);
  const autoSelectFirstOnNextLoadRef = useRef(false);

  const effectiveTheme = useTheme(theme);

  const appWindow = getCurrentWindow();
  const selectedFolderRef = useRef(selectedFolder);
  selectedFolderRef.current = selectedFolder;

  useEffect(() => {
    invoke<Settings>('get_settings')
      .then((s) => {
        setTheme(s.theme);
      })
      .catch(console.error);

    // Listen for setting changes from the settings window
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
    // Focus search input after a short delay to ensure it's rendered
    setTimeout(() => {
      searchInputRef.current?.focus();
    }, 100);
  }, []);

  // Reset selection, clear search, and scroll to top every time the window is shown/focused
  useEffect(() => {
    const unlisten = appWindow.listen('tauri://focus', () => {
      setSelectedClipId(null);
      setSearchQuery('');
      autoSelectFirstOnNextLoadRef.current = true;
      setWindowFocusCount((c) => c + 1);
    });
    return () => {
      unlisten.then((f) => f());
    };
  // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  // Focus search input AFTER React has rendered the cleared state (avoids stale DOM value)
  useEffect(() => {
    if (windowFocusCount > 0) {
      searchInputRef.current?.focus();
    }
  }, [windowFocusCount]);

  const openSettings = useCallback(async () => {
    // Check if settings window already exists
    const existingWin = await WebviewWindow.getByLabel('settings');
    if (existingWin) {
      try {
        await invoke('focus_window', { label: 'settings' });
      } catch (e) {
        console.error('Failed to focus settings window:', e);
        // Fallback to JS API if command fails (though command is preferred)
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
      decorations: false, // We have our own title bar in SettingsPanel
      center: true,
    });

    settingsWin.once('tauri://created', function () {});

    settingsWin.once('tauri://error', function (e) {
      console.error('Error creating settings window', e);
    });
  }, []);

  const loadClips = useCallback(
    async (folderId: string | null, append: boolean = false, searchQuery: string = '') => {
      try {
        setIsLoading(true);

        const currentOffset = append ? clips.length : 0;

        let data: AppClipboardItem[];

        if (searchQuery.trim()) {
          data = await invoke<AppClipboardItem[]>('search_clips', {
            query: searchQuery,
            filterId: folderId,
            limit: 20,
            offset: currentOffset,
          });
        } else {
          data = await invoke<AppClipboardItem[]>('get_clips', {
            filterId: folderId,
            limit: 20,
            offset: currentOffset,
            previewOnly: false,
          });
        }

        if (append) {
          setClips((prev) => {
            return [...prev, ...data];
          });
        } else {
          setClips(data);
          if (autoSelectFirstOnNextLoadRef.current) {
            autoSelectFirstOnNextLoadRef.current = false;
            setSelectedClipId(data[0]?.id ?? null);
          }
        }

        // If we got fewer than limit, no more clips
        setHasMore(data.length === 20);
      } catch (error) {
        console.error('Failed to load clips:', error);
      } finally {
        setIsLoading(false);
      }
    },
    [clips.length]
  );

  const loadFolders = useCallback(async () => {
    try {
      const data = await invoke<FolderItem[]>('get_folders');

      setFolders(data);
    } catch (error) {
      console.error('Failed to load folders:', error);
    }
  }, []);

  const refreshCurrentFolder = useCallback(() => {
    loadClips(selectedFolderRef.current, false, searchQuery);
  }, [loadClips, searchQuery]);

  const handleSearch = useCallback((query: string) => {
    setSearchQuery(query);
  }, []);

  useEffect(() => {
    loadFolders();
    if (searchQuery.trim()) {
      loadClips(selectedFolder, false, searchQuery);
    } else {
      loadClips(selectedFolder);
    }
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [selectedFolder, searchQuery]);

  // Handle global mouse events for simulated drag
  useEffect(() => {
    const handleGlobalMouseMove = (e: MouseEvent) => {
      const state = dragStateRef.current;

      // If we are already dragging, update position
      if (state.isDragging) {
        setDragPosition({ x: e.clientX, y: e.clientY });
        return;
      }

      // If we have a pending drag, check threshold
      if (state.pendingDrag) {
        const dx = e.clientX - state.pendingDrag.startX;
        const dy = e.clientY - state.pendingDrag.startY;
        const dist = Math.sqrt(dx * dx + dy * dy);

        if (dist > 5) {
          // Start actual drag
          setDraggingClipId(state.pendingDrag.clipId);
          setDragPosition({ x: e.clientX, y: e.clientY });
          dragStateRef.current.isDragging = true;
          dragStateRef.current.clipId = state.pendingDrag.clipId;
          dragStateRef.current.pendingDrag = null;
        }
      }
    };

    const handleGlobalMouseUp = (_: MouseEvent) => {
      // Always clear pending drag on mouse up
      if (dragStateRef.current.pendingDrag) {
        dragStateRef.current.pendingDrag = null;
      }

      if (dragStateRef.current.isDragging) {
        finishDrag();
      }
    };

    window.addEventListener('mousemove', handleGlobalMouseMove);
    window.addEventListener('mouseup', handleGlobalMouseUp);

    return () => {
      window.removeEventListener('mousemove', handleGlobalMouseMove);
      window.removeEventListener('mouseup', handleGlobalMouseUp);
    };
  }, []);

  const startDrag = (clipId: string, startX: number, startY: number) => {
    // Instead of starting immediately, set pending
    dragStateRef.current.pendingDrag = { clipId, startX, startY };
    dragStateRef.current.clipId = clipId;
    // We don't set state yet, avoiding re-render until threshold passed
  };

  const finishDrag = () => {
    if (dragStateRef.current.targetFolderId !== undefined && dragStateRef.current.clipId) {
      // We only move if targetFolderId was explicitly set by a hover event.
      // Wait, how do we distinguish "Not Hovering" vs "Hovering 'All' (null)"?
      // We will make ControlBar pass a specific sentinel for "No Target" when leaving?
      // Or simply: ControlBar tracks hover. If hover, it calls setDragTargetFolderId.
      // If we drop and dragTargetFolderId is valid, we move.
      // BUT 'null' is a valid folder ID (All).
      // Let's use a generic 'undefined' for "No Target".
    }

    // Actually, simpler:
    // When MouseUp happens, we check dragTargetFolderId state.
    // If it is NOT undefined, we execute move.

    // IMPORTANT: State updates in React are async. accessing `dragTargetFolderId` state inside event listener might be stale?
    // That's why we use `dragStateRef`.

    const { clipId, targetFolderId } = dragStateRef.current;
    if (clipId && targetFolderId !== undefined && targetFolderId !== 'NO_TARGET') {
      handleMoveClip(clipId, targetFolderId);
    }

    setDraggingClipId(null);
    setDragTargetFolderId(null);
    dragStateRef.current = {
      isDragging: false,
      clipId: null,
      targetFolderId: 'NO_TARGET',
      pendingDrag: null,
    };
  };

  const handleDragHover = (folderId: string | null) => {
    setDragTargetFolderId(folderId);
    dragStateRef.current.targetFolderId = folderId;
  };

  const handleDragLeave = () => {
    setDragTargetFolderId(null);
    dragStateRef.current.targetFolderId = 'NO_TARGET';
  };

  // Total History Count
  const [totalClipCount, setTotalClipCount] = useState(0);

  const refreshTotalCount = useCallback(async () => {
    try {
      const count = await invoke<number>('get_clipboard_history_size');
      setTotalClipCount(count);
    } catch (e) {
      console.error('Failed to get history size', e);
    }
  }, []);

  useEffect(() => {
    refreshTotalCount();
  }, [refreshTotalCount]);

  useEffect(() => {
    const unlistenClipboard = listen('clipboard-change', () => {
      refreshCurrentFolder();
      loadFolders(); // Refresh folders to get updated counts
      refreshTotalCount(); // Refresh total count
    });

    return () => {
      unlistenClipboard.then((unlisten) => {
        if (typeof unlisten === 'function') unlisten();
      });
    };
  }, [refreshCurrentFolder, loadFolders, refreshTotalCount]);

  useKeyboard({
    onClose: () => { if (!editingClip) appWindow.hide(); },
    onSearch: () => {
      if (editingClip) return;
      setShowSearch(true);
      setTimeout(() => {
        searchInputRef.current?.focus();
      }, 50);
    },
    onDelete: () => { if (!editingClip) handleDelete(selectedClipId); },
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
      if (selectedClipId && !editingClip) {
        handlePaste(selectedClipId);
      }
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

  const handleDelete = async (clipId: string | null) => {
    if (!clipId) return;
    if (isDeletingRef.current) return;
    isDeletingRef.current = true;
    try {
      // Folder items are hard-deleted directly (soft-deleted folder items can never be cleaned up by bulk clear)
      const isInFolder = clips.find((c) => c.id === clipId)?.folder_id != null;
      await invoke('delete_clip', { id: clipId, hardDelete: isInFolder });
      setClips((prev) => prev.filter((c) => c.id !== clipId));
      setSelectedClipId(null);
      // Refresh counts
      loadFolders();
      refreshTotalCount();
      toast.success('Clip deleted');
    } catch (error) {
      console.error('Failed to delete clip:', error);
      toast.error('Failed to delete clip');
    } finally {
      isDeletingRef.current = false;
    }
  };

  const handlePaste = async (clipId: string) => {
    try {
      const clip = clips.find((c) => c.id === clipId);
      if (clip && clip.clip_type === 'image') {
        try {
           // clip.content is Base64 for images (from get_clips in commands.rs)
           const blob = base64ToBlob(clip.content, 'image/png');
           await navigator.clipboard.write([new ClipboardItem({ [blob.type]: blob })]);
        } catch (e) {
           console.error("Frontend clipboard write failed", e);
        }
      }

      await invoke('paste_clip', { id: clipId });
      // Backend now handles hiding and auto-pasting (and database update)
    } catch (error) {
      console.error('Failed to paste clip:', error);
    }
  };

  const handleTogglePin = async (clipId: string) => {
    try {
      const isPinned = await invoke<boolean>('toggle_pin', { id: clipId });
      setClips((prev) =>
        prev.map((c) => (c.id === clipId ? { ...c, is_pinned: isPinned } : c))
      );
      toast.success(isPinned ? 'Pinned' : 'Unpinned');
      // Reload to re-sort pinned items to top
      refreshCurrentFolder();
    } catch (error) {
      console.error('Failed to toggle pin:', error);
      toast.error('Failed to pin clip');
    }
  };

  const handleCopy = async (clipId: string) => {
    try {
      const clip = clips.find((c) => c.id === clipId);
      if (clip && clip.clip_type === 'image') {
        const blob = base64ToBlob(clip.content, 'image/png');
        await navigator.clipboard.write([new ClipboardItem({ [blob.type]: blob })]);
      }

      await invoke('paste_clip', { id: clipId });

      toast.success('Copied to clipboard');
    } catch (error) {
      console.error('Failed to copy clip:', error);
      toast.error('Failed to copy');
    }
  };

  const handleCreateFolder = async (name: string, color: string | null) => {
    try {
      await invoke('create_folder', { name, icon: null, color });
      await loadFolders();
    } catch (error) {
      console.error('Failed to create folder:', error);
    }
  };

  const loadMore = useCallback(() => {
    if (hasMore && !isLoading) {
      loadClips(selectedFolder, true, searchQuery);
    }
  }, [hasMore, isLoading, selectedFolder, loadClips, searchQuery]);

  const handleMoveClip = async (clipId: string, folderId: string | null) => {
    try {
      await invoke('move_to_folder', { clipId, folderId });

      // Update local state to reflect the move
      if (selectedFolder) {
        // If we are in a specific folder (not All)
        if (folderId !== selectedFolder) {
          // If moved to a different folder, remove from current view
          setClips((prev) => prev.filter((c) => c.id !== clipId));
        }
      } else {
        // If we are in "All clips" view, just update the folder_id
        setClips((prev) => prev.map((c) => (c.id === clipId ? { ...c, folder_id: folderId } : c)));
      }
      // Refresh counts after move
      loadFolders();
      refreshTotalCount();
    } catch (error) {
      console.error('Failed to move clip:', error);
    }
  };

  // Context Menu State
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

  // Edit clip before paste
  const [editingClip, setEditingClip] = useState<AppClipboardItem | null>(null);

  // Folder hover preview state
  const [previewFolder, setPreviewFolder] = useState<string | null | undefined>(undefined);
  const [previewClips, setPreviewClips] = useState<AppClipboardItem[]>([]);
  const [isPreviewLoading, setIsPreviewLoading] = useState(false);
  const previewCacheRef = useRef<Map<string, AppClipboardItem[]>>(new Map());
  const previewRequestIdRef = useRef(0); // Track latest request to ignore stale responses
  const previewEndTimerRef = useRef<ReturnType<typeof setTimeout> | null>(null);

  const cancelPreviewEnd = useCallback(() => {
    if (previewEndTimerRef.current) {
      clearTimeout(previewEndTimerRef.current);
      previewEndTimerRef.current = null;
    }
  }, []);

  const clearPreview = useCallback(() => {
    cancelPreviewEnd();
    previewRequestIdRef.current++; // Invalidate any in-flight requests
    setPreviewFolder(undefined);
    setPreviewClips([]);
    setIsPreviewLoading(false);
  }, [cancelPreviewEnd]);

  const handleFolderHover = useCallback(async (folderId: string | null) => {
    cancelPreviewEnd();
    const requestId = ++previewRequestIdRef.current;

    setPreviewFolder(folderId);

    // Check cache first
    const cacheKey = folderId ?? '__all__';
    const cached = previewCacheRef.current.get(cacheKey);
    if (cached) {
      setPreviewClips(cached);
      setIsPreviewLoading(false);
      return;
    }

    setIsPreviewLoading(true);
    try {
      const data = await invoke<AppClipboardItem[]>('get_clips', {
        filterId: folderId,
        limit: 20,
        offset: 0,
        previewOnly: false,
      });
      // Only apply if this is still the latest request
      if (requestId !== previewRequestIdRef.current) return;
      previewCacheRef.current.set(cacheKey, data);
      setPreviewClips(data);
    } catch (error) {
      if (requestId !== previewRequestIdRef.current) return;
      console.error('Failed to load preview clips:', error);
    } finally {
      if (requestId === previewRequestIdRef.current) {
        setIsPreviewLoading(false);
      }
    }
  }, [cancelPreviewEnd]);

  const handleFolderHoverEnd = useCallback(() => {
    // Delay ending preview so user can move mouse down to clip list
    cancelPreviewEnd();
    previewEndTimerRef.current = setTimeout(() => {
      clearPreview();
    }, 300);
  }, [cancelPreviewEnd, clearPreview]);

  const handlePreviewListEnter = useCallback(() => {
    // Mouse entered clip list while previewing — keep preview alive
    cancelPreviewEnd();
  }, [cancelPreviewEnd]);

  const handlePreviewListLeave = useCallback(() => {
    clearPreview();
  }, [clearPreview]);

  // Invalidate preview cache when clips change (new copy, delete, move, etc.)
  useEffect(() => {
    previewCacheRef.current.clear();
  }, [clips, folders]);

  const isPreviewing = previewFolder !== undefined;

  const handleContextMenu = useCallback(
    (e: React.MouseEvent, type: 'card' | 'folder', itemId: string) => {
      e.preventDefault();
      setContextMenu({
        type,
        x: e.clientX,
        y: e.clientY,
        itemId,
      });
    },
    []
  );

  const handleCloseContextMenu = useCallback(() => {
    setContextMenu(null);
  }, []);

  const handleEditBeforePaste = useCallback((clipId: string) => {
    const clip = clipsRef.current.find((c) => c.id === clipId);
    if (clip && clip.clip_type !== 'image') {
      setEditingClip(clip);
    }
  }, []);

  const handlePasteEdited = useCallback(async (editedText: string) => {
    setEditingClip(null);
    try {
      await invoke('paste_text', { content: editedText });
    } catch (error) {
      console.error('Failed to paste edited text:', error);
      toast.error('Failed to paste');
    }
  }, []);

  // Updated Create Folder to handle Rename
  const handleCreateOrRenameFolder = async (name: string, color: string | null) => {
    if (folderModalMode === 'create') {
      await handleCreateFolder(name, color);
      toast.success(`Folder "${name}" created`);
      setShowAddFolderModal(false);
      setNewFolderName('');
    } else if (folderModalMode === 'rename' && editingFolderId) {
      try {
        await invoke('rename_folder', { id: editingFolderId, name, color });
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

  const handleReorderFolders = async (folderIds: string[]) => {
    try {
      await invoke('reorder_folders', { folderIds });
      await loadFolders();
    } catch (error) {
      console.error('Failed to reorder folders:', error);
    }
  };

  const handleDeleteFolder = async (folderId: string) => {
    if (!folderId) return;
    try {
      await invoke('delete_folder', { id: folderId });
      if (selectedFolder === folderId) {
        setSelectedFolder(null);
      }
      await loadFolders();
      refreshTotalCount();
      toast.success('Folder deleted');
    } catch (error) {
      console.error('Failed to delete folder:', error);
      toast.error('Failed to delete folder');
    }
  };

  return (
    <div className="relative h-screen w-full overflow-hidden">
      {/*
      Background Layer with Blur:
      The Limitation: Standard CSS backdrop-filter:
      blur() works by blurring elements behind the div. However, on a transparent app window,
      the "element behind" is the OS desktop, which the browser engine cannot see or blur for security and
      performance reasons. This is why you see "no blur" right now—it's trying to blur transparent pixels.
      */}
      <div
        className="absolute inset-0"
        style={{
          backgroundColor: 'transparent',
          backdropFilter: 'blur(2px)',
        }}
      />

      {/* Content Container */}
      <div className="relative h-full w-full" style={{ padding: `${LAYOUT.WINDOW_PADDING}px` }}>
        <div className="flex h-full w-full flex-col overflow-hidden rounded-[12px] border border-border/10 bg-background/80 font-sans text-foreground shadow-[0_0_24px_rgba(0,0,0,0.2)] dark:shadow-[0_0_24px_rgba(0,0,0,0.4)]">
          {draggingClipId && (() => {
            const dragClip = clips.find((c) => c.id === draggingClipId);
            return dragClip ? (
              <DragPreview
                clip={dragClip}
                position={dragPosition}
              />
            ) : null;
          })()}

          {contextMenu && (
            <ContextMenu
              x={contextMenu.x}
              y={contextMenu.y}
              onClose={handleCloseContextMenu}
              options={
                contextMenu.type === 'card'
                  ? [
                      ...(selectedFolder ? [{
                        label: clips.find((c) => c.id === contextMenu.itemId)?.is_pinned ? 'Unpin' : 'Pin',
                        onClick: () => handleTogglePin(contextMenu.itemId),
                      }] : []),
                      ...(clips.find((c) => c.id === contextMenu.itemId)?.clip_type !== 'image'
                        ? [{ label: 'Chỉnh sửa trước khi paste', onClick: () => handleEditBeforePaste(contextMenu.itemId) }]
                        : []),
                      ...folders.map((folder) => ({
                        label: `Move to "${folder.name}"`,
                        onClick: () => handleMoveClip(contextMenu.itemId, folder.id),
                      })),
                      {
                        label: 'Remove from folder',
                        onClick: () => handleMoveClip(contextMenu.itemId, null),
                      },
                      {
                        label: 'Delete',
                        danger: true,
                        onClick: () => handleDelete(contextMenu.itemId),
                      },
                    ]
                  : [
                      {
                        label: 'Rename',
                        onClick: () => {
                          setFolderModalMode('rename');
                          setEditingFolderId(contextMenu.itemId);
                          const folder = folders.find((f) => f.id === contextMenu.itemId);
                          setNewFolderName(folder ? folder.name : '');
                          setEditingFolderColor(folder?.color ?? null);
                          setShowAddFolderModal(true);
                        },
                      },
                      {
                        label: 'Change color',
                        onClick: () => {
                          setFolderModalMode('rename');
                          setEditingFolderId(contextMenu.itemId);
                          const folder = folders.find((f) => f.id === contextMenu.itemId);
                          setNewFolderName(folder ? folder.name : '');
                          setEditingFolderColor(folder?.color ?? null);
                          setShowAddFolderModal(true);
                        },
                      },
                      {
                        label: 'Delete',
                        danger: true,
                        onClick: () => handleDeleteFolder(contextMenu.itemId),
                      },
                    ]
              }
            />
          )}

          <EditClipModal
            clip={editingClip}
            onPaste={handlePasteEdited}
            onClose={() => setEditingClip(null)}
          />

          <ControlBar
            ref={searchInputRef}
            folders={folders}
            selectedFolder={selectedFolder}
            onSelectFolder={setSelectedFolder}
            showSearch={showSearch}
            searchQuery={searchQuery}
            onSearchChange={handleSearch}
            onSearchClick={() => {
              if (showSearch) {
                handleSearch(''); // Clear search when closing
              }
              setShowSearch(!showSearch);
            }}
            onAddClick={() => {
              setFolderModalMode('create');
              setNewFolderName('');
              setShowAddFolderModal(true);
            }}
            onMoreClick={openSettings}
            // Simulated Drag Props
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
          />

          <main
            className="no-scrollbar relative flex-1"
            onMouseEnter={isPreviewing ? handlePreviewListEnter : undefined}
            onMouseLeave={isPreviewing ? handlePreviewListLeave : undefined}
          >
            <ClipList
              clips={isPreviewing ? previewClips : clips}
              isLoading={isPreviewing ? isPreviewLoading : isLoading}
              hasMore={isPreviewing ? false : hasMore}
              selectedClipId={selectedClipId}
              onSelectClip={setSelectedClipId}
              onPaste={handlePaste}
              onCopy={handleCopy}
              onPin={handleTogglePin}
              showPin={!!(isPreviewing ? previewFolder : selectedFolder)}
              onLoadMore={isPreviewing ? () => {} : loadMore}
              resetScrollKey={isPreviewing ? undefined : windowFocusCount}
              // Simulated Drag Props
              onDragStart={startDrag}
              onCardContextMenu={(e, clipId) => handleContextMenu(e, 'card', clipId)}
              isPreviewing={isPreviewing}
            />

            {/* Add/Rename Folder Modal Overlay */}
            <FolderModal
              isOpen={showAddFolderModal}
              mode={folderModalMode}
              initialName={newFolderName}
              initialColor={folderModalMode === 'rename' ? editingFolderColor : null}
              onClose={() => {
                setShowAddFolderModal(false);
                setNewFolderName('');
              }}
              onSubmit={handleCreateOrRenameFolder}
            />

          </main>
          <Toaster richColors position="bottom-center" theme={effectiveTheme} />
        </div>
      </div>
    </div>
  );
}

export default App;
