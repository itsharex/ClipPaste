import { useRef, useEffect, useState } from 'react';
import { ClipboardItem } from '../types';
import { ClipCard } from './ClipCard';

interface ClipListProps {
  clips: ClipboardItem[];
  isLoading: boolean;
  hasMore: boolean;
  selectedClipId: string | null;
  onSelectClip: (clipId: string) => void;
  onPaste: (clipId: string) => void;
  onCopy: (clipId: string) => void;
  onPin: (clipId: string) => void;
  showPin?: boolean;
  onLoadMore: () => void;
  resetScrollKey?: number;
  onNativeDragStart?: (e: React.DragEvent, clip: ClipboardItem) => void;
  onCardContextMenu?: (e: React.MouseEvent, clipId: string) => void;
  isPreviewing?: boolean;
}

export function ClipList({
  clips,
  isLoading,
  hasMore,
  selectedClipId,
  onSelectClip,
  onPaste,
  onCopy,
  onPin,
  showPin,
  onLoadMore,
  resetScrollKey,
  onNativeDragStart,
  onCardContextMenu,
  isPreviewing,
}: ClipListProps) {
  const containerRef = useRef<HTMLDivElement>(null);
  const [fadeIn, setFadeIn] = useState(false);
  const prevClipsKeyRef = useRef('');

  // Detect when the clip list changes entirely (folder switch) and trigger a subtle fade
  const clipsKey = clips.slice(0, 5).map(c => c.id).join(',');
  useEffect(() => {
    if (prevClipsKeyRef.current && clipsKey !== prevClipsKeyRef.current) {
      setFadeIn(true);
      const t = setTimeout(() => setFadeIn(false), 200);
      return () => clearTimeout(t);
    }
    prevClipsKeyRef.current = clipsKey;
  }, [clipsKey]);

  // Scroll selected card into view when navigating with arrow keys
  useEffect(() => {
    if (!selectedClipId || !containerRef.current) return;
    const card = containerRef.current.querySelector(`[data-clip-id="${selectedClipId}"]`);
    if (card) {
      card.scrollIntoView({ behavior: 'smooth', block: 'nearest', inline: 'nearest' });
    }
  }, [selectedClipId]);

  // Scroll to start when window is reopened
  useEffect(() => {
    if (resetScrollKey === undefined || resetScrollKey === 0) return;
    if (containerRef.current) {
      containerRef.current.scrollLeft = 0;
    }
  }, [resetScrollKey]);

  // Native onScroll handler for infinite scroll
  const handleScroll = () => {
    if (!containerRef.current || !hasMore || isLoading) return;
    const { scrollLeft, scrollWidth, clientWidth } = containerRef.current;
    if (scrollLeft + clientWidth >= scrollWidth - 300) {
      onLoadMore();
    }
  };

  // Map vertical mouse wheel to horizontal scroll
  const handleWheel = (e: React.WheelEvent) => {
    if (containerRef.current && e.deltaY !== 0) {
      containerRef.current.scrollLeft += e.deltaY * 1;
    }
  };

  if (isLoading && clips.length === 0) {
    return (
      <div className="flex h-full w-full items-center justify-center">
        <div className="flex flex-col items-center gap-3">
          <div className="h-8 w-8 animate-spin rounded-full border-2 border-primary/30 border-t-primary" />
          <p className="text-sm text-muted-foreground">Loading clips...</p>
        </div>
      </div>
    );
  }

  if (clips.length === 0) {
    return (
      <div className="flex h-full w-full flex-col items-center justify-center p-8 text-center">
        <h3 className="mb-2 text-lg font-semibold text-gray-400">No clips yet</h3>
        <p className="max-w-xs text-sm text-gray-500">
          Copy something to your clipboard and it will appear here.
        </p>
      </div>
    );
  }

  return (
    <div
      ref={containerRef}
      className={`no-scrollbar flex h-full w-full flex-1 items-center gap-4 overflow-x-auto overflow-y-hidden px-4 transition-opacity duration-200${isPreviewing ? ' opacity-80' : ''}${fadeIn ? ' opacity-0' : ''}`}
      onScroll={handleScroll}
      onWheel={handleWheel}
      style={{
        scrollBehavior: 'auto',
      }}
    >
      {clips.map((clip) => (
        <ClipCard
          key={clip.id}
          clip={clip}
          isSelected={selectedClipId === clip.id}
          onSelect={() => onSelectClip(clip.id)}
          onPaste={() => onPaste(clip.id)}
          onCopy={() => onCopy(clip.id)}
          onPin={() => onPin(clip.id)}
          showPin={showPin}
          onNativeDragStart={onNativeDragStart}
          onContextMenu={(e: React.MouseEvent) => onCardContextMenu?.(e, clip.id)}
        />
      ))}

      {/* Loading indicator at the end */}
      {isLoading && clips.length > 0 && (
        <div className="flex h-full min-w-[100px] items-center justify-center">
          <div className="h-6 w-6 animate-spin rounded-full border-2 border-primary/30 border-t-primary" />
        </div>
      )}

      {/* Spacer end */}
      <div className="h-full min-w-[20px] flex-shrink-0" />
    </div>
  );
}
