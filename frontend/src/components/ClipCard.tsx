import { ClipboardItem } from '../types';
import { clsx } from 'clsx';
import { useMemo, memo, useState } from 'react';
import { LAYOUT, TOTAL_COLUMN_WIDTH, PREVIEW_CHAR_LIMIT } from '../constants';
import { Copy, Check, Pin } from 'lucide-react';

interface ClipCardProps {
  clip: ClipboardItem;
  isSelected: boolean;
  onSelect: () => void;
  onPaste: () => void;
  onCopy: () => void;
  onPin: () => void;
  showPin?: boolean;
  onDragStart: (clipId: string, startX: number, startY: number) => void;
  onContextMenu?: (e: React.MouseEvent) => void;
}

export const ClipCard = memo(function ClipCard({
  clip,
  isSelected,
  onSelect,
  onPaste,
  onCopy,
  onPin,
  showPin,
  onDragStart,
  onContextMenu,
}: ClipCardProps) {
  const [copied, setCopied] = useState(false);
  const title = clip.source_app || clip.clip_type.toUpperCase();

  // Memoize the content rendering
  const renderedContent = useMemo(() => {
    if (clip.clip_type === 'image') {
      return (
        <div className="flex h-full w-full select-none items-center justify-center">
          <img
            src={`data:image/png;base64,${clip.content}`}
            alt="Clipboard Image"
            className="max-h-full max-w-full object-contain"
          />
        </div>
      );
    } else {
      return (
        <pre className="whitespace-pre-wrap break-all font-mono text-[13px] leading-tight text-foreground">
          <span>{clip.content.substring(0, PREVIEW_CHAR_LIMIT)}</span>
        </pre>
      );
    }
  }, [clip.clip_type, clip.content]);

  // Generate distinct color based on source app name
  const getAppGradient = (name: string) => {
    let hash = 0;
    for (let i = 0; i < name.length; i++) {
      hash = name.charCodeAt(i) + ((hash << 5) - hash);
    }
    const gradients = [
      'bg-gradient-to-r from-red-500 to-rose-400',
      'bg-gradient-to-r from-orange-500 to-amber-400',
      'bg-gradient-to-r from-amber-500 to-yellow-400',
      'bg-gradient-to-r from-green-500 to-emerald-400',
      'bg-gradient-to-r from-emerald-500 to-teal-400',
      'bg-gradient-to-r from-teal-500 to-cyan-400',
      'bg-gradient-to-r from-cyan-500 to-sky-400',
      'bg-gradient-to-r from-sky-500 to-blue-400',
      'bg-gradient-to-r from-blue-500 to-indigo-400',
      'bg-gradient-to-r from-indigo-500 to-violet-400',
      'bg-gradient-to-r from-violet-500 to-purple-400',
      'bg-gradient-to-r from-purple-500 to-fuchsia-400',
      'bg-gradient-to-r from-fuchsia-500 to-pink-400',
      'bg-gradient-to-r from-pink-500 to-rose-400',
      'bg-gradient-to-r from-rose-500 to-red-400',
    ];
    return gradients[Math.abs(hash) % gradients.length];
  };

  const headerColor = useMemo(() => getAppGradient(title), [title]);

  const handleMouseDown = (e: React.MouseEvent) => {
    // Only left click
    if (e.button !== 0) return;
    onDragStart(clip.id, e.clientX, e.clientY);
  };

  const handleContextMenu = (e: React.MouseEvent) => {
    e.preventDefault();
    onContextMenu?.(e);
  };

  return (
    <div
      data-clip-id={clip.id}
      style={{
        width: TOTAL_COLUMN_WIDTH - LAYOUT.CARD_GAP,
        height: LAYOUT.WINDOW_HEIGHT - LAYOUT.CONTROL_BAR_HEIGHT - LAYOUT.CARD_VERTICAL_PADDING * 2,
      }}
      className="flex-shrink-0"
    >
      <div
        onMouseDown={handleMouseDown}
        onClick={onSelect}
        onDoubleClick={onPaste}
        onContextMenu={handleContextMenu}
        className={clsx(
          'relative flex h-full w-full cursor-pointer select-none flex-col overflow-hidden rounded-xl border border-border bg-card shadow-lg',
          'transition-all duration-200 ease-out',
          isSelected
            ? 'z-10 scale-[1.04] -translate-y-1 ring-[3px] ring-blue-500/80 shadow-xl shadow-blue-500/20'
            : 'hover:scale-[1.02] hover:-translate-y-[3px] hover:-rotate-[0.5deg] hover:shadow-xl hover:shadow-primary/10 hover:ring-2 hover:ring-primary/40',
          'group'
        )}
      >
        <div className={clsx(headerColor, 'flex flex-shrink-0 items-center gap-2 px-2 py-1.5')}>
          {clip.source_icon && (
            <img
              src={`data:image/png;base64,${clip.source_icon}`}
              alt=""
              className="h-4 w-4 object-contain"
            />
          )}
          <span className="flex-1 truncate text-[11px] font-bold uppercase tracking-wider text-foreground shadow-sm">
            {title}
          </span>
          {showPin && (
            <button
              onClick={(e) => {
                e.stopPropagation();
                onPin();
              }}
              className={clsx(
                'rounded-md p-1 transition-opacity duration-150 hover:bg-black/10',
                clip.is_pinned ? 'opacity-100' : 'opacity-0 group-hover:opacity-100'
              )}
              title={clip.is_pinned ? 'Unpin' : 'Pin'}
            >
              <Pin size={14} className={clsx(
                clip.is_pinned ? 'text-amber-400 fill-amber-400' : 'text-foreground/70 hover:text-foreground'
              )} />
            </button>
          )}
          <button
            onClick={(e) => {
              e.stopPropagation();
              onCopy();
              setCopied(true);
              setTimeout(() => setCopied(false), 2000);
            }}
            className="rounded-md p-1 opacity-0 transition-opacity duration-150 hover:bg-black/10 group-hover:opacity-100"
            title="Copy to clipboard"
          >
            {copied ? (
              <Check size={14} className="text-emerald-500" />
            ) : (
              <Copy size={14} className="text-foreground/70 hover:text-foreground" />
            )}
          </button>
        </div>

        <div className="relative flex-1 overflow-hidden bg-card p-2">
          {renderedContent}
          <div className="pointer-events-none absolute bottom-0 left-0 right-0 h-12 bg-gradient-to-t from-card/100 to-card/30" />
        </div>

        <div className="absolute bottom-0 left-0 right-0 bg-gradient-to-t from-card via-card/100 to-transparent/0 px-3 py-1.5">
          <span className="text-[11px] font-medium text-muted-foreground/50">
            {clip.clip_type === 'image'
              ? `Image (${Math.round((clip.content.length * 0.75) / 1024)}KB)`
              : `${clip.content.length} characters`}
          </span>
        </div>
      </div>
    </div>
  );
});
