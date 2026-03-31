import { ClipboardItem } from '../types';
import { clsx } from 'clsx';
import { useMemo, memo, useState, useRef } from 'react';
import { LAYOUT, TOTAL_COLUMN_WIDTH, PREVIEW_CHAR_LIMIT } from '../constants';
import { Copy, Check, Pin, Link, Mail, Palette, FolderOpen, StickyNote, Image as ImageIcon } from 'lucide-react';

interface ClipCardProps {
  clip: ClipboardItem;
  isSelected: boolean;
  onSelect: () => void;
  onPaste: () => void;
  onCopy: () => void;
  onPin: () => void;
  showPin?: boolean;
  onNativeDragStart?: (e: React.DragEvent, clip: ClipboardItem) => void;
  onContextMenu?: (e: React.MouseEvent) => void;
}

/** Try to extract domain from a URL string */
function extractDomain(url: string): string | null {
  try {
    const u = new URL(url.trim());
    return u.hostname.replace(/^www\./, '');
  } catch {
    return null;
  }
}

/** Subtype badge config */
const SUBTYPE_CONFIG: Record<string, { icon: typeof Link; label: string; color: string }> = {
  url: { icon: Link, label: 'URL', color: 'text-blue-400' },
  email: { icon: Mail, label: 'Email', color: 'text-emerald-400' },
  color: { icon: Palette, label: 'Color', color: 'text-pink-400' },
  path: { icon: FolderOpen, label: 'Path', color: 'text-amber-400' },
};

export const ClipCard = memo(function ClipCard({
  clip,
  isSelected,
  onSelect,
  onPaste,
  onCopy,
  onPin,
  showPin,
  onNativeDragStart,
  onContextMenu,
}: ClipCardProps) {
  const [copied, setCopied] = useState(false);
  const title = clip.source_app || clip.clip_type.toUpperCase();

  // Memoize the content rendering — now subtype-aware
  const renderedContent = useMemo(() => {
    if (clip.clip_type === 'image') {
      return (
        <div className="flex h-full w-full select-none items-center justify-center rounded-md bg-black/20 p-1">
          {clip.content ? (
            <img
              src={`data:image/png;base64,${clip.content}`}
              alt="Clipboard Image"
              className="max-h-full max-w-full rounded object-contain shadow-md"
            />
          ) : (
            <div className="flex flex-col items-center gap-1 text-muted-foreground/50">
              <ImageIcon size={24} />
              <span className="text-[10px]">Image</span>
            </div>
          )}
        </div>
      );
    }

    // Color subtype — large swatch + text
    if (clip.subtype === 'color') {
      const color = clip.content.trim();
      return (
        <div className="flex h-full w-full flex-col items-center justify-center gap-2">
          <div
            className="h-14 w-14 rounded-xl border-2 border-white/20 shadow-lg"
            style={{ backgroundColor: color }}
          />
          <span className="font-mono text-[13px] font-semibold text-foreground/80">{color}</span>
        </div>
      );
    }

    // URL subtype — show domain badge + full URL readable
    if (clip.subtype === 'url') {
      const domain = extractDomain(clip.content);
      return (
        <div className="flex h-full w-full flex-col gap-1.5">
          {domain && (
            <div className="flex items-center gap-1.5 rounded-md bg-blue-500/10 px-1.5 py-1">
              <Link size={12} className="flex-shrink-0 text-blue-400" />
              <span className="truncate text-[12px] font-semibold text-blue-400">{domain}</span>
            </div>
          )}
          <pre className="flex-1 whitespace-pre-wrap break-all font-mono text-[11px] leading-snug text-foreground/70">
            {clip.content.substring(0, PREVIEW_CHAR_LIMIT)}
          </pre>
        </div>
      );
    }

    // Email subtype — show email badge + full content
    if (clip.subtype === 'email') {
      return (
        <div className="flex h-full w-full flex-col gap-1.5">
          <div className="flex items-center gap-1.5 rounded-md bg-emerald-500/10 px-1.5 py-1">
            <Mail size={12} className="flex-shrink-0 text-emerald-400" />
            <span className="text-[11px] font-semibold text-emerald-400">Email</span>
          </div>
          <pre className="whitespace-pre-wrap break-all font-mono text-[12px] leading-snug text-foreground/90">
            {clip.content.trim()}
          </pre>
        </div>
      );
    }

    // Path subtype — show full path, word-wrap friendly
    if (clip.subtype === 'path') {
      const content = clip.content.trim();
      return (
        <div className="flex h-full w-full flex-col gap-1.5">
          <div className="flex items-center gap-1.5 rounded-md bg-amber-500/10 px-1.5 py-1">
            <FolderOpen size={12} className="flex-shrink-0 text-amber-400" />
            <span className="text-[11px] font-semibold text-amber-400">Path</span>
          </div>
          <pre className="whitespace-pre-wrap break-all font-mono text-[12px] leading-snug text-foreground/90">
            {content}
          </pre>
        </div>
      );
    }

    // Default text
    return (
      <pre className="whitespace-pre-wrap break-all font-mono text-[13px] leading-tight text-foreground">
        <span>{clip.content.substring(0, PREVIEW_CHAR_LIMIT)}</span>
      </pre>
    );
  }, [clip.clip_type, clip.content, clip.subtype]);

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

  // Subtype badge for header
  const subtypeBadge = useMemo(() => {
    if (!clip.subtype || !SUBTYPE_CONFIG[clip.subtype]) return null;
    const cfg = SUBTYPE_CONFIG[clip.subtype];
    const Icon = cfg.icon;
    return (
      <span className={clsx('flex items-center gap-0.5 rounded px-1 py-0.5 text-[9px] font-bold uppercase tracking-wider bg-black/15', cfg.color)}>
        <Icon size={9} />
        {cfg.label}
      </span>
    );
  }, [clip.subtype]);

  const cardRef = useRef<HTMLDivElement>(null);

  const handleNativeDragStart = (e: React.DragEvent) => {
    // Set data for external drop targets (other apps)
    if (clip.clip_type === 'image') {
      try {
        const byteChars = atob(clip.content);
        const byteArray = new Uint8Array(byteChars.length);
        for (let i = 0; i < byteChars.length; i++) {
          byteArray[i] = byteChars.charCodeAt(i);
        }
        const blob = new Blob([byteArray], { type: 'image/png' });
        const file = new File([blob], 'clipboard-image.png', { type: 'image/png' });
        e.dataTransfer.items.add(file);
      } catch {
        e.dataTransfer.setData('text/plain', '[Image]');
      }
    } else {
      e.dataTransfer.setData('text/plain', clip.content);
    }
    e.dataTransfer.effectAllowed = 'copyMove';

    // Use the card itself as the drag ghost, offset to center on cursor
    if (cardRef.current) {
      const rect = cardRef.current.getBoundingClientRect();
      e.dataTransfer.setDragImage(cardRef.current, e.clientX - rect.left, e.clientY - rect.top);
    }

    onNativeDragStart?.(e, clip);
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
        ref={cardRef}
        draggable
        onDragStart={handleNativeDragStart}
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
        {/* Header */}
        <div className={clsx(headerColor, 'flex flex-shrink-0 items-center gap-1.5 px-2 py-1.5')}>
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
          {subtypeBadge}
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

        {/* Content */}
        <div className="relative flex-1 overflow-hidden bg-card p-2">
          {renderedContent}
          <div className="pointer-events-none absolute bottom-0 left-0 right-0 h-12 bg-gradient-to-t from-card/100 to-card/30" />
        </div>

        {/* Note banner — shown above footer when clip has a note */}
        {clip.note && (
          <div className="flex items-center gap-1 border-t border-border/20 bg-amber-500/5 px-2 py-0.5">
            <StickyNote size={9} className="flex-shrink-0 text-amber-400/70" />
            <span className="truncate text-[10px] italic text-amber-400/70">{clip.note}</span>
          </div>
        )}

        {/* Footer */}
        <div className="flex items-center justify-between bg-card/80 px-2.5 py-1">
          <span className="text-[10px] font-medium text-muted-foreground/40">
            {clip.clip_type === 'image'
              ? `${Math.round((clip.content.length * 0.75) / 1024)}KB`
              : `${clip.content.length} chars`}
          </span>
          <span className="flex items-center gap-2 text-[10px] text-muted-foreground/35">
            {clip.paste_count > 0 && (
              <span className="tabular-nums" title="Times pasted">×{clip.paste_count}</span>
            )}
          </span>
        </div>
      </div>
    </div>
  );
});
