import { useCallback, useRef, useState } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { ClipboardItem as AppClipboardItem } from '../types';

interface UseDragDropOpts {
  handleMoveClip: (clipId: string, folderId: string | null) => Promise<void>;
}

export function useDragDrop(opts: UseDragDropOpts) {
  const { handleMoveClip } = opts;

  const [draggingClipId, setDraggingClipId] = useState<string | null>(null);
  const [dragTargetFolderId, setDragTargetFolderId] = useState<string | null>(null);

  const dragStateRef = useRef({
    isDragging: false,
    clipId: null as string | null,
    targetFolderId: null as string | null,
  });

  const isDraggingExternalRef = useRef(false);

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
  }, [handleMoveClip]);

  return {
    draggingClipId,
    dragTargetFolderId,
    dragStateRef,
    isDraggingExternalRef,
    handleDragHover,
    handleDragLeave,
    handleNativeDragStart,
    handleNativeDragEnd,
  };
}
