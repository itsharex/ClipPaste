import { useEffect, useState, useCallback, useRef, useMemo } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';
import { getCurrentWindow } from '@tauri-apps/api/window';
import { WebviewWindow } from '@tauri-apps/api/webviewWindow';
import { ClipboardItem as AppClipboardItem, FolderItem, Settings, ClipType } from './types';
import { ClipList } from './components/ClipList';
import { ControlBar } from './components/ControlBar';
import { ContextMenu } from './components/ContextMenu';
import { FolderModal } from './components/FolderModal';
import { EditClipModal } from './components/EditClipModal';
import { NoteModal } from './components/NoteModal';
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
  const [contentTypeFilter, setContentTypeFilter] = useState<ClipType | null>(null);
  const [selectedClipId, setSelectedClipId] = useState<string | null>(null);
  const [isLoading, setIsLoading] = useState(true);
  const [hasMore, setHasMore] = useState(true);
  const [theme, setTheme] = useState('system');

  // Simulated Drag State
  const [draggingClipId, setDraggingClipId] = useState<string | null>(null);
  const [dragTargetFolderId, setDragTargetFolderId] = useState<string | null>(null);

  // Add Folder Modal State
  const [showAddFolderModal, setShowAddFolderModal] = useState(false);
  const [newFolderName, setNewFolderName] = useState('');

  // Ref for drag state — used by HTML5 drag handlers to avoid stale closures
  const dragStateRef = useRef({
    isDragging: false,
    clipId: null as string | null,
    targetFolderId: null as string | null,
  });

  const searchInputRef = useRef<HTMLInputElement>(null);
  const isDeletingRef = useRef(false);
  const isDraggingExternalRef = useRef(false);
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

  // Reset selection, clear search, reload clips, and scroll to top every time the window is shown/focused
  useEffect(() => {
    const unlisten = appWindow.listen('tauri://focus', () => {
      setSelectedClipId(null);
      setSelectedFolder(null);
      setSearchQuery('');
      setSearchInput('');

      setContentTypeFilter(null);
      setPreviewFolder(undefined);
      autoSelectFirstOnNextLoadRef.current = true;
      setWindowFocusCount((c) => c + 1);
      // Force reload All clips with empty query
      loadClipsRef.current(null, false, '');
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

  // Monotonic counter to discard stale responses from older queries
  const loadGenRef = useRef(0);

  const loadClips = useCallback(
    async (folderId: string | null, append: boolean = false, searchQuery: string = '') => {
      const thisGen = ++loadGenRef.current;

      try {
        if (clips.length === 0) setIsLoading(true);

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

        // Discard if a newer query has been fired since
        if (loadGenRef.current !== thisGen) return;

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

        setHasMore(data.length === 20);
      } catch (error) {
        console.error('Failed to load clips:', error);
      } finally {
        if (loadGenRef.current === thisGen) {
          setIsLoading(false);
        }
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

  // Stable ref so the clipboard listener never re-subscribes
  const refreshCurrentFolderRef = useRef(refreshCurrentFolder);
  refreshCurrentFolderRef.current = refreshCurrentFolder;

  // Stable ref for loadClips — used in focus handler to bypass stale closures
  const loadClipsRef = useRef(loadClips);
  loadClipsRef.current = loadClips;
  const debouncedFolderRefreshRef = useRef<() => void>(() => {});

  const [searchInput, setSearchInput] = useState('');
  const searchTimerRef = useRef<ReturnType<typeof setTimeout> | null>(null);

  const handleSearch = useCallback((query: string) => {
    setSearchInput(query);
    setPreviewFolder(undefined);
    if (searchTimerRef.current) clearTimeout(searchTimerRef.current);
    searchTimerRef.current = setTimeout(() => {
      setSearchQuery(query);
    }, 100);
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

  // === Unified HTML5 Drag System ===
  // Handles both internal folder moves AND external drag-copy to other apps.

  const handleDragHover = (folderId: string | null) => {
    setDragTargetFolderId(folderId);
    dragStateRef.current.targetFolderId = folderId;
  };

  const handleDragLeave = () => {
    setDragTargetFolderId(null);
    dragStateRef.current.targetFolderId = 'NO_TARGET';
  };

  const handleNativeDragStart = useCallback((_e: React.DragEvent, clip: AppClipboardItem) => {
    isDraggingExternalRef.current = true;
    invoke('set_dragging', { dragging: true }).catch(console.error);
    setDraggingClipId(clip.id);
    dragStateRef.current.isDragging = true;
    dragStateRef.current.clipId = clip.id;
  }, []);

  const handleNativeDragEnd = useCallback(() => {
    // Check if we dropped on a folder target
    const { clipId, targetFolderId } = dragStateRef.current;
    if (clipId && targetFolderId !== undefined && targetFolderId !== 'NO_TARGET') {
      handleMoveClip(clipId, targetFolderId);
    }

    isDraggingExternalRef.current = false;
    invoke('set_dragging', { dragging: false }).catch(console.error);
    setDraggingClipId(null);
    setDragTargetFolderId(null);
    dragStateRef.current = {
      isDragging: false,
      clipId: null,
      targetFolderId: 'NO_TARGET',
    };
  // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

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

  // Debounced folder/count refresh — avoids hammering DB on rapid copies
  const folderRefreshTimerRef = useRef<ReturnType<typeof setTimeout> | null>(null);
  const debouncedFolderRefresh = useCallback(() => {
    if (folderRefreshTimerRef.current) clearTimeout(folderRefreshTimerRef.current);
    folderRefreshTimerRef.current = setTimeout(() => {
      loadFolders();
      refreshTotalCount();
    }, 500);
  }, [loadFolders, refreshTotalCount]);
  debouncedFolderRefreshRef.current = debouncedFolderRefresh;

  // Subscribe ONCE — uses refs so the callback is always fresh without re-subscribing
  useEffect(() => {
    const unlistenClipboard = listen<{ clip_type?: string }>('clipboard-change', (event) => {
      refreshCurrentFolderRef.current();
      debouncedFolderRefreshRef.current();
      // Visual feedback: clip saved
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

  const handleCreateFolder = async (name: string, color: string | null, icon: string | null) => {
    try {
      await invoke('create_folder', { name, icon, color });
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
  const [editingFolderIcon, setEditingFolderIcon] = useState<string | null>(null);

  // Edit clip before paste
  const [editingClip, setEditingClip] = useState<AppClipboardItem | null>(null);

  // Note modal state
  const [noteModalClipId, setNoteModalClipId] = useState<string | null>(null);
  const [noteModalInitial, setNoteModalInitial] = useState('');

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

  // Invalidate preview cache only when clip/folder structure changes (not count updates)
  const clipIdsKey = clips.map(c => c.id).join(',');
  const folderIdsKey = folders.map(f => f.id).join(',');
  useEffect(() => {
    previewCacheRef.current.clear();
  }, [clipIdsKey, folderIdsKey]);

  const isPreviewing = previewFolder !== undefined;

  // Smart content type matching — backend only stores "text" and "image",
  // so we detect url/file/html/rtf from content for filtering purposes
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
    // "text" = everything that's text but NOT url/file
    if (filter === 'text') {
      return clip.clip_type === 'text'
        && !/^https?:\/\/\S+$/i.test(clip.content.trim())
        && !/^[a-zA-Z]:\\/.test(clip.content.trim());
    }
    return clip.clip_type === filter;
  }, []);

  // Filter clips by content type (client-side)
  const filteredClips = useMemo(() => {
    if (!contentTypeFilter) return clips;
    return clips.filter((c) => matchesContentType(c, contentTypeFilter));
  }, [clips, contentTypeFilter, matchesContentType]);

  const filteredPreviewClips = useMemo(() => {
    if (!contentTypeFilter) return previewClips;
    return previewClips.filter((c) => matchesContentType(c, contentTypeFilter));
  }, [previewClips, contentTypeFilter, matchesContentType]);

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

  const handleEditNote = useCallback((clipId: string) => {
    const clip = clipsRef.current.find((c) => c.id === clipId);
    setNoteModalClipId(clipId);
    setNoteModalInitial(clip?.note || '');
  }, []);

  const handleSaveNote = useCallback(async (clipId: string, note: string | null) => {
    setNoteModalClipId(null);
    try {
      await invoke('update_note', { id: clipId, note });
      setClips((prev) =>
        prev.map((c) => (c.id === clipId ? { ...c, note } : c))
      );
      toast.success(note ? 'Note saved' : 'Note removed');
    } catch (error) {
      console.error('Failed to update note:', error);
      toast.error('Failed to save note');
    }
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
    <div className="relative h-screen w-full overflow-hidden" onDragEnd={handleNativeDragEnd}>
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
        <div className="flex h-full w-full flex-col overflow-hidden rounded-[12px] border border-border/10 bg-background/80 text-foreground shadow-[0_4px_32px_rgba(0,0,0,0.15)] dark:shadow-[0_4px_32px_rgba(0,0,0,0.5)]">
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
                      {
                        label: clips.find((c) => c.id === contextMenu.itemId)?.note ? 'Edit note' : 'Add note',
                        onClick: () => handleEditNote(contextMenu.itemId),
                      },
                      {
                        label: 'Delete',
                        danger: true,
                        onClick: () => handleDelete(contextMenu.itemId),
                      },
                    ]
                  : (() => {
                      return [
                        {
                          label: 'Edit folder',
                          onClick: () => {
                            setFolderModalMode('rename');
                            setEditingFolderId(contextMenu.itemId);
                            const folder = folders.find((f) => f.id === contextMenu.itemId);
                            setNewFolderName(folder ? folder.name : '');
                            setEditingFolderColor(folder?.color ?? null);
                            setEditingFolderIcon(folder?.icon ?? null);
                            setShowAddFolderModal(true);
                          },
                        },
                        {
                          label: 'Delete',
                          danger: true,
                          onClick: () => {
                            const folder = folders.find((f) => f.id === contextMenu.itemId);
                            if (window.confirm(`Delete folder "${folder?.name}"? Clips inside will be moved to All.`)) {
                              handleDeleteFolder(contextMenu.itemId);
                            }
                          },
                        },
                      ];
                    })()
              }
            />
          )}

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
              onSelectClip={setSelectedClipId}
              onPaste={handlePaste}
              onCopy={handleCopy}
              onPin={handleTogglePin}
              showPin={!!(isPreviewing ? previewFolder : selectedFolder)}
              onLoadMore={isPreviewing ? () => {} : loadMore}
              resetScrollKey={isPreviewing ? undefined : windowFocusCount}
              // Simulated Drag Props
              onNativeDragStart={handleNativeDragStart}
              onCardContextMenu={(e, clipId) => handleContextMenu(e, 'card', clipId)}
              isPreviewing={isPreviewing}
              isSearching={!!searchQuery.trim()}
            />


          </main>

          {/* Folder Modal — outside <main> to avoid overflow-hidden clipping */}
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
