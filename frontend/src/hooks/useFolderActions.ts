import { useCallback, useRef, useState } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { FolderItem } from '../types';
import { toast } from 'sonner';

interface UseFolderActionsOpts {
  selectedFolder: string | null;
  setSelectedFolder: (v: string | null) => void;
  setClips: React.Dispatch<React.SetStateAction<import('../types').ClipboardItem[]>>;
}

export function useFolderActions(opts: UseFolderActionsOpts) {
  const { selectedFolder, setSelectedFolder, setClips } = opts;

  const [folders, setFolders] = useState<FolderItem[]>([]);
  const [totalClipCount, setTotalClipCount] = useState(0);

  const loadFolders = useCallback(async () => {
    try {
      const data = await invoke<FolderItem[]>('get_folders');
      setFolders(data);
    } catch (error) {
      console.error('Failed to load folders:', error);
    }
  }, []);

  const refreshTotalCount = useCallback(async () => {
    try {
      const count = await invoke<number>('get_clipboard_history_size');
      setTotalClipCount(count);
    } catch (e) {
      console.error('Failed to get history size', e);
    }
  }, []);

  const handleCreateFolder = async (name: string, color: string | null, icon: string | null) => {
    try {
      await invoke('create_folder', { name, icon, color });
      await loadFolders();
    } catch (error) {
      console.error('Failed to create folder:', error);
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

  const handleReorderFolders = async (folderIds: string[]) => {
    try {
      await invoke('reorder_folders', { folderIds });
      await loadFolders();
    } catch (error) {
      console.error('Failed to reorder folders:', error);
    }
  };

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

  // Debounced folder/count refresh — avoids hammering DB on rapid copies
  const folderRefreshTimerRef = useRef<ReturnType<typeof setTimeout> | null>(null);
  const debouncedFolderRefresh = useCallback(() => {
    if (folderRefreshTimerRef.current) clearTimeout(folderRefreshTimerRef.current);
    folderRefreshTimerRef.current = setTimeout(() => {
      loadFolders();
      refreshTotalCount();
    }, 500);
  }, [loadFolders, refreshTotalCount]);

  return {
    folders,
    setFolders,
    totalClipCount,
    loadFolders,
    refreshTotalCount,
    handleCreateFolder,
    handleDeleteFolder,
    handleReorderFolders,
    handleMoveClip,
    debouncedFolderRefresh,
  };
}
