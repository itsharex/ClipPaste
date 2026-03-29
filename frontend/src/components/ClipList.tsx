import { useRef, useEffect, useCallback } from 'react';
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
  onDragStart: (clipId: string, startX: number, startY: number) => void;
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
  onDragStart,
  onCardContextMenu,
  isPreviewing,
}: ClipListProps) {
  const containerRef = useRef<HTMLDivElement>(null);

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

    // We check native scroll properties
    const { scrollLeft, scrollWidth, clientWidth } = containerRef.current;

    // If scrolled within 300px of the end
    if (scrollLeft + clientWidth >= scrollWidth - 300) {
      onLoadMore();
    }
  };

  // Smooth horizontal scroll with momentum
  const scrollVelocityRef = useRef(0);
  const scrollAnimRef = useRef<number | null>(null);

  const animateScroll = useCallback(() => {
    const container = containerRef.current;
    if (!container) return;

    container.scrollLeft += scrollVelocityRef.current;
    scrollVelocityRef.current *= 0.92; // friction — decay factor

    if (Math.abs(scrollVelocityRef.current) > 0.5) {
      scrollAnimRef.current = requestAnimationFrame(animateScroll);
    } else {
      scrollVelocityRef.current = 0;
      scrollAnimRef.current = null;
    }
  }, []);

  // Register native wheel listener (non-passive) so preventDefault works
  useEffect(() => {
    const container = containerRef.current;
    if (!container) return;

    const onWheel = (e: WheelEvent) => {
      e.preventDefault();
      const delta = e.deltaY !== 0 ? e.deltaY : e.deltaX;
      scrollVelocityRef.current += delta * 0.8;
      if (!scrollAnimRef.current) {
        scrollAnimRef.current = requestAnimationFrame(animateScroll);
      }
    };

    container.addEventListener('wheel', onWheel, { passive: false });
    return () => {
      container.removeEventListener('wheel', onWheel);
      if (scrollAnimRef.current) cancelAnimationFrame(scrollAnimRef.current);
    };
  }, [animateScroll]);

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
      className={`no-scrollbar flex h-full w-full flex-1 items-center gap-4 overflow-x-auto overflow-y-hidden px-4${isPreviewing ? ' opacity-80' : ''}`}
      onScroll={handleScroll}
      style={{
        // Smooth scrolling disabled per user request
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
          onDragStart={onDragStart}
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
