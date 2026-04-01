import { useCallback, useRef } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { ClipboardItem as AppClipboardItem } from '../types';
import { base64ToBlob } from '../utils';
import { PAGE_SIZE } from '../constants';
import { toast } from 'sonner';

interface UseClipActionsOpts {
  clips: AppClipboardItem[];
  setClips: React.Dispatch<React.SetStateAction<AppClipboardItem[]>>;
  setIsLoading: (v: boolean) => void;
  setHasMore: (v: boolean) => void;
  setSelectedClipId: (v: string | null) => void;
  setEditingClip: (v: AppClipboardItem | null) => void;
  setNoteModalClipId: (v: string | null) => void;
  setNoteModalInitial: (v: string) => void;
  loadFolders: () => Promise<void>;
  refreshTotalCount: () => Promise<void>;
  refreshCurrentFolder: () => void;
}

export function useClipActions(opts: UseClipActionsOpts) {
  const {
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
    refreshCurrentFolder,
  } = opts;

  const isDeletingRef = useRef(false);
  const clipsRef = useRef(clips);
  clipsRef.current = clips;

  // Monotonic counter to discard stale responses from older queries
  const loadGenRef = useRef(0);
  const autoSelectFirstOnNextLoadRef = useRef(false);

  const loadClips = useCallback(
    async (folderId: string | null, append: boolean = false, searchOverride: string = '') => {
      const thisGen = ++loadGenRef.current;

      try {
        if (clipsRef.current.length === 0) setIsLoading(true);

        const currentOffset = append ? clipsRef.current.length : 0;

        let data: AppClipboardItem[];

        if (searchOverride.trim()) {
          data = await invoke<AppClipboardItem[]>('search_clips', {
            query: searchOverride,
            filterId: folderId,
            limit: PAGE_SIZE,
            offset: currentOffset,
          });
        } else {
          data = await invoke<AppClipboardItem[]>('get_clips', {
            filterId: folderId,
            limit: PAGE_SIZE,
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

        setHasMore(data.length === PAGE_SIZE);
      } catch (error) {
        console.error('Failed to load clips:', error);
      } finally {
        if (loadGenRef.current === thisGen) {
          setIsLoading(false);
        }
      }
    },
    [setClips, setIsLoading, setHasMore, setSelectedClipId]
  );

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
          console.error('Frontend clipboard write failed', e);
        }
      }

      await invoke('paste_clip', { id: clipId });
      // Backend now handles hiding and auto-pasting (and database update)
    } catch (error) {
      console.error('Failed to paste clip:', error);
    }
  };

  const handleCopy = async (clipId: string) => {
    try {
      const clip = clipsRef.current.find((c) => c.id === clipId);
      if (clip && clip.clip_type === 'image') {
        const blob = base64ToBlob(clip.content, 'image/png');
        await navigator.clipboard.write([new ClipboardItem({ [blob.type]: blob })]);
      }

      await invoke('copy_clip', { id: clipId });

      toast.success('Copied to clipboard');
    } catch (error) {
      console.error('Failed to copy clip:', error);
      toast.error('Failed to copy');
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

  const handleEditBeforePaste = useCallback((clipId: string) => {
    const clip = clipsRef.current.find((c) => c.id === clipId);
    if (clip && clip.clip_type !== 'image') {
      setEditingClip(clip);
    }
  }, [setEditingClip]);

  const handlePasteEdited = useCallback(async (editedText: string) => {
    setEditingClip(null);
    try {
      await invoke('paste_text', { content: editedText });
    } catch (error) {
      console.error('Failed to paste edited text:', error);
      toast.error('Failed to paste');
    }
  }, [setEditingClip]);

  const handlePastePlainText = useCallback(async (clipId: string) => {
    const clip = clipsRef.current.find((c) => c.id === clipId);
    if (!clip || clip.clip_type === 'image') return;
    try {
      await invoke('paste_text', { content: clip.content });
    } catch (error) {
      console.error('Failed to paste as plain text:', error);
      toast.error('Failed to paste');
    }
  }, []);

  const handleEditNote = useCallback((clipId: string) => {
    const clip = clipsRef.current.find((c) => c.id === clipId);
    setNoteModalClipId(clipId);
    setNoteModalInitial(clip?.note || '');
  }, [setNoteModalClipId, setNoteModalInitial]);

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
  }, [setClips, setNoteModalClipId]);

  return {
    loadClips,
    loadGenRef,
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
  };
}
