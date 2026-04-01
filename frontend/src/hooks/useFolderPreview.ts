import { useCallback, useEffect, useMemo, useRef, useState } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { ClipboardItem as AppClipboardItem, FolderItem } from '../types';

interface UseFolderPreviewOpts {
  clips: AppClipboardItem[];
  folders: FolderItem[];
}

export function useFolderPreview(opts: UseFolderPreviewOpts) {
  const { clips, folders } = opts;

  const [previewFolder, setPreviewFolder] = useState<string | null | undefined>(undefined);
  const [previewClips, setPreviewClips] = useState<AppClipboardItem[]>([]);
  const [isPreviewLoading, setIsPreviewLoading] = useState(false);
  const previewCacheRef = useRef<Map<string, AppClipboardItem[]>>(new Map());
  const previewRequestIdRef = useRef(0);
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
  const clipIdsKey = useMemo(() => clips.map(c => c.id).join(','), [clips]);
  const folderIdsKey = useMemo(() => folders.map(f => f.id).join(','), [folders]);
  useEffect(() => {
    previewCacheRef.current.clear();
  }, [clipIdsKey, folderIdsKey]);

  const isPreviewing = previewFolder !== undefined;

  return {
    previewFolder,
    setPreviewFolder,
    previewClips,
    isPreviewLoading,
    isPreviewing,
    clearPreview,
    handleFolderHover,
    handleFolderHoverEnd,
    handlePreviewListEnter,
    handlePreviewListLeave,
  };
}
