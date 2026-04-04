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
  // Stable callback refs — avoids re-creating closures per card
  showPin?: boolean;
  onLoadMore: () => void;
  resetScrollKey?: number;
  onNativeDragStart?: (e: React.DragEvent, clip: ClipboardItem) => void;
  onCardContextMenu?: (e: React.MouseEvent, clipId: string) => void;
  isPreviewing?: boolean;
  isSearching?: boolean;
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
  folderMap,
  selectedFolder,
  searchQuery,
}: ClipListProps) {
  const containerRef = useRef<HTMLDivElement>(null);
  const [staggerKey, setStaggerKey] = useState(0);
  const prevClipsKeyRef = useRef('');

  // Stable callback refs — prevent re-creating inline closures per card on every render
  const onSelectClipRef = useRef(onSelectClip);
  onSelectClipRef.current = onSelectClip;
  const onPasteRef = useRef(onPaste);
  onPasteRef.current = onPaste;
  const onCopyRef = useRef(onCopy);
  onCopyRef.current = onCopy;
  const onPinRef = useRef(onPin);
  onPinRef.current = onPin;
  const onCardContextMenuRef = useRef(onCardContextMenu);
  onCardContextMenuRef.current = onCardContextMenu;
  const onNativeDragStartRef = useRef(onNativeDragStart);
  onNativeDragStartRef.current = onNativeDragStart;

  // Stable callbacks that read from refs — never change identity
  const stableOnSelect = useCallback((clipId: string, e?: React.MouseEvent) => onSelectClipRef.current(clipId, e), []);
  const stableOnPaste = useCallback((clipId: string) => onPasteRef.current(clipId), []);
  const stableOnCopy = useCallback((clipId: string) => onCopyRef.current(clipId), []);
  const stableOnPin = useCallback((clipId: string) => onPinRef.current(clipId), []);
  const stableOnContextMenu = useCallback((e: React.MouseEvent, clipId: string) => onCardContextMenuRef.current?.(e, clipId), []);
  const stableOnDragStart = useCallback((e: React.DragEvent, clip: ClipboardItem) => onNativeDragStartRef.current?.(e, clip), []);

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
    overscan: 5,
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
              No clips found matching your search. Try different keywords or use fewer words.
            </p>
          </>
        ) : (
          <>
            <h3 className="mb-2 text-lg font-semibold text-gray-400">No clips yet</h3>
            <p className="max-w-xs text-sm text-gray-500">
              Copy something to your clipboard and it will appear here.
            </p>
            <div className="mt-4 flex flex-col gap-1.5 text-xs text-gray-500/70">
              <span><kbd className="rounded bg-muted/40 px-1.5 py-0.5 font-mono text-[10px]">Enter</kbd> to paste · <kbd className="rounded bg-muted/40 px-1.5 py-0.5 font-mono text-[10px]">↑↓</kbd> navigate</span>
              <span><kbd className="rounded bg-muted/40 px-1.5 py-0.5 font-mono text-[10px]">E</kbd> edit · <kbd className="rounded bg-muted/40 px-1.5 py-0.5 font-mono text-[10px]">P</kbd> pin · <kbd className="rounded bg-muted/40 px-1.5 py-0.5 font-mono text-[10px]">Ctrl+Del</kbd> delete</span>
            </div>
          </>
        )}
      </div>
    );
  }

  return (
    <div
      ref={containerRef}
      role="listbox"
      aria-label="Clipboard history"
      aria-orientation="horizontal"
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
                onSelect={(e) => stableOnSelect(clip.id, e)}
                onPaste={() => stableOnPaste(clip.id)}
                onCopy={() => stableOnCopy(clip.id)}
                onPin={() => stableOnPin(clip.id)}
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
                onNativeDragStart={stableOnDragStart}
                onContextMenu={(e: React.MouseEvent) => stableOnContextMenu(e, clip.id)}
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
