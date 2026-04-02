import { useRef, useEffect, useState, useCallback, useMemo } from 'react';
import { useVirtualizer } from '@tanstack/react-virtual';
import { clsx } from 'clsx';
import { ClipboardItem } from '../types';
import { ClipCard } from './ClipCard';
import { LAYOUT, TOTAL_COLUMN_WIDTH } from '../constants';

interface ClipListProps {
  clips: ClipboardItem[];
  isLoading: boolean;
  hasMore: boolean;
  selectedClipId: string | null;
  selectedClipIds?: Set<string>;
  onSelectClip: (clipId: string, e?: React.MouseEvent) => void;
  onPaste: (clipId: string) => void;
  onCopy: (clipId: string) => void;
  onPin: (clipId: string) => void;
  showPin?: boolean;
  onLoadMore: () => void;
  resetScrollKey?: number;
  onNativeDragStart?: (e: React.DragEvent, clip: ClipboardItem) => void;
  onCardContextMenu?: (e: React.MouseEvent, clipId: string) => void;
  isPreviewing?: boolean;
  isSearching?: boolean;
  isSearchPending?: boolean;
  folderMap?: Record<string, string>;
  selectedFolder?: string | null;
  searchQuery?: string;
}

export function ClipList({
  clips,
  isLoading,
  hasMore,
  selectedClipId,
  selectedClipIds,
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
  isSearching,
  isSearchPending,
  folderMap,
  selectedFolder,
  searchQuery,
}: ClipListProps) {
  const containerRef = useRef<HTMLDivElement>(null);
  const [staggerKey, setStaggerKey] = useState(0);
  const prevClipsKeyRef = useRef('');

  // Detect when the clip list changes entirely and trigger stagger animation
  const clipsKey = clips.slice(0, 5).map(c => c.id).join(',');
  useEffect(() => {
    if (prevClipsKeyRef.current && clipsKey !== prevClipsKeyRef.current) {
      setStaggerKey((k) => k + 1);
    }
    prevClipsKeyRef.current = clipsKey;
  }, [clipsKey]);

  // Multi-select order: map clip id → display index (0-based) among selected clips
  const multiSelectOrder = useMemo(() => {
    if (!selectedClipIds || selectedClipIds.size <= 1) return new Map<string, number>();
    const map = new Map<string, number>();
    let idx = 0;
    for (const clip of clips) {
      if (selectedClipIds.has(clip.id)) {
        map.set(clip.id, idx++);
      }
    }
    return map;
  }, [clips, selectedClipIds]);

  // Virtual list — horizontal
  const virtualizer = useVirtualizer({
    count: clips.length,
    getScrollElement: () => containerRef.current,
    estimateSize: () => TOTAL_COLUMN_WIDTH,
    horizontal: true,
    overscan: 3,
  });

  // Scroll selected card into view when navigating with arrow keys
  useEffect(() => {
    if (!selectedClipId) return;
    const index = clips.findIndex(c => c.id === selectedClipId);
    if (index >= 0) {
      virtualizer.scrollToIndex(index, { align: 'auto', behavior: 'smooth' });
    }
  // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [selectedClipId]);

  // Scroll to start when window reopened or clip list changes (search, folder switch)
  useEffect(() => {
    // RAF ensures DOM has rendered before resetting scroll
    requestAnimationFrame(() => {
      if (containerRef.current) {
        containerRef.current.scrollLeft = 0;
      }
    });
  }, [resetScrollKey, clipsKey]);

  // Infinite scroll — load more when near the end
  const handleScroll = useCallback(() => {
    if (!containerRef.current || !hasMore || isLoading) return;
    const { scrollLeft, scrollWidth, clientWidth } = containerRef.current;
    if (scrollLeft + clientWidth >= scrollWidth - 300) {
      onLoadMore();
    }
  }, [hasMore, isLoading, onLoadMore]);

  // Convert vertical wheel → horizontal scroll. Trackpad horizontal gestures work natively.
  const handleWheel = useCallback((e: React.WheelEvent) => {
    if (!containerRef.current) return;
    // Only intercept vertical scroll (mouse wheel / trackpad vertical swipe)
    // Let native horizontal trackpad gestures pass through untouched
    if (e.deltaY !== 0 && e.deltaX === 0) {
      e.preventDefault();
      // Mouse wheel: deltaMode=0 with large steps (~100px), needs higher multiplier
      // Trackpad: deltaMode=0 with small steps (~1-30px), needs lower multiplier
      const isLikelyMouse = Math.abs(e.deltaY) >= 50;
      const multiplier = isLikelyMouse ? 2.5 : 0.5;
      containerRef.current.scrollLeft += e.deltaY * multiplier;
    }
  }, []);

  // Skeleton cards only while search is pending (not yet resolved)
  if (isSearchPending) {
    const skeletonGradients = [
      'from-violet-500/40 to-purple-400/40',
      'from-cyan-500/40 to-sky-400/40',
      'from-emerald-500/40 to-teal-400/40',
      'from-orange-500/40 to-amber-400/40',
      'from-pink-500/40 to-rose-400/40',
      'from-blue-500/40 to-indigo-400/40',
      'from-green-500/40 to-emerald-400/40',
      'from-fuchsia-500/40 to-pink-400/40',
    ];
    const skeletonCount = Math.min(
      Math.ceil(window.innerWidth / TOTAL_COLUMN_WIDTH) + 1,
      skeletonGradients.length
    );
    return (
      <div className="no-scrollbar flex h-full w-full flex-1 items-center gap-4 overflow-x-auto overflow-y-hidden px-4">
        {Array.from({ length: skeletonCount }).map((_, i) => (
          <div
            key={i}
            className="animate-skeleton-in flex-shrink-0"
            style={{
              width: TOTAL_COLUMN_WIDTH - LAYOUT.CARD_GAP,
              height: LAYOUT.WINDOW_HEIGHT - LAYOUT.CONTROL_BAR_HEIGHT - LAYOUT.CARD_VERTICAL_PADDING * 2,
              animationDelay: `${i * 40}ms`,
            }}
          >
            <div className="relative flex h-full w-full flex-col overflow-hidden rounded-xl border border-border/30 bg-card/60 shadow-lg">
              <div className={`flex items-center gap-2 px-3 py-2 bg-gradient-to-r ${skeletonGradients[i]}`}>
                <div className="h-4 w-4 rounded-full bg-white/20" />
                <div className="h-3 w-20 rounded-full bg-white/20" />
              </div>
              <div className="flex-1 space-y-3 p-3">
                <div className="skeleton-shimmer h-3 w-[85%] rounded-full bg-muted/15" />
                <div className="skeleton-shimmer h-3 w-[60%] rounded-full bg-muted/15" style={{ animationDelay: '0.1s' }} />
                <div className="skeleton-shimmer h-3 w-[72%] rounded-full bg-muted/15" style={{ animationDelay: '0.2s' }} />
                <div className="skeleton-shimmer h-3 w-[45%] rounded-full bg-muted/15" style={{ animationDelay: '0.3s' }} />
              </div>
              <div className="px-3 py-2">
                <div className="h-2.5 w-16 rounded-full bg-muted/10" />
              </div>
            </div>
          </div>
        ))}
      </div>
    );
  }

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
        {isSearching ? (
          <>
            <h3 className="mb-2 text-lg font-semibold text-gray-400">No results</h3>
            <p className="max-w-xs text-sm text-gray-500">
              No clips found matching your search.
            </p>
          </>
        ) : (
          <>
            <h3 className="mb-2 text-lg font-semibold text-gray-400">No clips yet</h3>
            <p className="max-w-xs text-sm text-gray-500">
              Copy something to your clipboard and it will appear here.
            </p>
          </>
        )}
      </div>
    );
  }

  return (
    <div
      ref={containerRef}
      className={`no-scrollbar flex h-full w-full flex-1 overflow-x-auto overflow-y-hidden${isPreviewing ? ' opacity-80' : ''}`}
      onScroll={handleScroll}
      onWheel={handleWheel}
      style={{ scrollSnapType: 'x proximity', scrollPaddingLeft: LAYOUT.SIDE_PADDING, scrollBehavior: 'smooth' }}
    >
      {/* Virtual spacer — the full scrollable width */}
      <div
        className="relative h-full"
        style={{
          width: virtualizer.getTotalSize() + LAYOUT.SIDE_PADDING * 2,
          minWidth: '100%',
        }}
      >
        {virtualizer.getVirtualItems().map((virtualItem, viewIndex) => {
          const clip = clips[virtualItem.index];
          return (
            <div
              key={clip.id}
              className={clsx('absolute flex items-center', isSearching ? undefined : 'animate-stagger-in')}
              style={{
                top: 0,
                left: virtualItem.start + LAYOUT.SIDE_PADDING,
                width: virtualItem.size,
                height: '100%',
                scrollSnapAlign: 'start',
                ...(isSearching ? {} : { animationDelay: `${viewIndex * 30}ms` }),
              }}
              data-stagger-key={staggerKey}
            >
              <ClipCard
                clip={clip}
                isSelected={selectedClipId === clip.id}
                isMultiSelected={selectedClipIds?.has(clip.id) ?? false}
                multiSelectIndex={selectedClipIds?.has(clip.id) ? multiSelectOrder.get(clip.id) : undefined}
                onSelect={(e) => onSelectClip(clip.id, e)}
                onPaste={() => onPaste(clip.id)}
                onCopy={() => onCopy(clip.id)}
                onPin={() => onPin(clip.id)}
                showPin={showPin}
                folderName={
                  isSearching && folderMap && selectedFolder
                    ? (clip.folder_id !== selectedFolder
                        ? (clip.folder_id ? folderMap[clip.folder_id] : 'All')
                        : null)
                    : (isSearching && folderMap && !selectedFolder && clip.folder_id
                        ? folderMap[clip.folder_id]
                        : null)
                }
                onNativeDragStart={onNativeDragStart}
                onContextMenu={(e: React.MouseEvent) => onCardContextMenu?.(e, clip.id)}
                searchQuery={searchQuery}
              />
            </div>
          );
        })}
      </div>

      {/* Loading indicator at the end */}
      {isLoading && clips.length > 0 && (
        <div className="flex h-full min-w-[100px] items-center justify-center">
          <div className="h-6 w-6 animate-spin rounded-full border-2 border-primary/30 border-t-primary" />
        </div>
      )}
    </div>
  );
}
