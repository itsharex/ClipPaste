import { ClipboardItem } from '../types';
import { clsx } from 'clsx';
import { useMemo, memo, useState, useRef, useEffect, useCallback } from 'react';
import { convertFileSrc, invoke } from '@tauri-apps/api/core';
import { LAYOUT, TOTAL_COLUMN_WIDTH, PREVIEW_CHAR_LIMIT } from '../constants';
import { Copy, Check, Pin, Link, Mail, Palette, FolderOpen, StickyNote, Image as ImageIcon, Folder, ShieldAlert } from 'lucide-react';
import { formatDistanceToNowStrict } from 'date-fns';

/** Image component that tries asset protocol first, falls back to base64 via get_clip */
function ImageWithFallback({ src, clipId, alt, className }: { src: string; clipId: string; alt: string; className: string }) {
  const [imgSrc, setImgSrc] = useState(src);
  const [failed, setFailed] = useState(false);
  const handleError = useCallback(() => {
    if (failed) return;
    setFailed(true);
    invoke<{ content: string; clip_type: string }>('get_clip', { id: clipId })
      .then((clip) => {
        if (clip.clip_type === 'image' && clip.content) {
          setImgSrc(`data:image/png;base64,${clip.content}`);
        }
      })
      .catch(() => {});
  }, [clipId, failed]);
  return <img src={imgSrc} alt={alt} className={className} onError={handleError} />;
}

interface ClipCardProps {
  clip: ClipboardItem;
  isSelected: boolean;
  isMultiSelected?: boolean;
  multiSelectIndex?: number;
  onSelect: (e?: React.MouseEvent) => void;
  onPaste: () => void;
  onCopy: () => void;
  onPin: () => void;
  showPin?: boolean;
  folderName?: string | null;
  onNativeDragStart?: (e: React.DragEvent, clip: ClipboardItem) => void;
  onContextMenu?: (e: React.MouseEvent) => void;
  searchQuery?: string;
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

/** Highlight search matches in text */
function HighlightText({ text, query }: { text: string; query?: string }) {
  if (!query?.trim()) return <>{text}</>;
  const words = query.toLowerCase().split(/\s+/).filter(Boolean);
  // Build regex from words, escape special chars
  const escaped = words.map(w => w.replace(/[.*+?^${}()|[\]\\]/g, '\\$&'));
  const regex = new RegExp(`(${escaped.join('|')})`, 'gi');
  const parts = text.split(regex);
  return (
    <>
      {parts.map((part, i) =>
        words.some(w => part.toLowerCase() === w)
          ? <mark key={i} className="bg-yellow-400/30 text-foreground rounded-sm">{part}</mark>
          : part
      )}
    </>
  );
}

/** Format relative time: "2m", "1h", "3d" */
function relativeTime(isoDate: string): string {
  try {
    return formatDistanceToNowStrict(new Date(isoDate), { addSuffix: false })
      .replace(' seconds', 's').replace(' second', 's')
      .replace(' minutes', 'm').replace(' minute', 'm')
      .replace(' hours', 'h').replace(' hour', 'h')
      .replace(' days', 'd').replace(' day', 'd')
      .replace(' months', 'mo').replace(' month', 'mo')
      .replace(' years', 'y').replace(' year', 'y');
  } catch {
    return '';
  }
}

/** Parse image size from metadata JSON */
function getImageSizeFromMeta(metadata: string | null): string | null {
  if (!metadata) return null;
  try {
    const meta = JSON.parse(metadata);
    if (meta.size_bytes) {
      const kb = meta.size_bytes / 1024;
      return kb >= 1024 ? `${(kb / 1024).toFixed(1)}MB` : `${Math.round(kb)}KB`;
    }
  } catch {}
  return null;
}

export const ClipCard = memo(function ClipCard({
  clip,
  isSelected,
  isMultiSelected,
  multiSelectIndex,
  onSelect,
  onPaste,
  onCopy,
  onPin,
  showPin,
  folderName,
  onNativeDragStart,
  onContextMenu,
  searchQuery,
}: ClipCardProps) {
  const [copied, setCopied] = useState(false);
  const copiedTimerRef = useRef<ReturnType<typeof setTimeout> | null>(null);
  useEffect(() => {
    return () => {
      if (copiedTimerRef.current) clearTimeout(copiedTimerRef.current);
    };
  }, []);
  const title = clip.source_app || clip.clip_type.toUpperCase();

  // Memoize the content rendering — now subtype-aware
  const renderedContent = useMemo(() => {
    if (clip.clip_type === 'image') {
      const imageSrc = clip.content ? convertFileSrc(clip.content) : '';
      return (
        <div className="flex h-full w-full select-none items-center justify-center rounded-md bg-black/20 p-1">
          {clip.content ? (
            <ImageWithFallback
              src={imageSrc}
              clipId={clip.id}
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
            <HighlightText text={clip.content.substring(0, PREVIEW_CHAR_LIMIT)} query={searchQuery} />
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
            <HighlightText text={clip.content.trim()} query={searchQuery} />
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
            <HighlightText text={content} query={searchQuery} />
          </pre>
        </div>
      );
    }

    // Default text
    return (
      <pre className="whitespace-pre-wrap break-all font-mono text-[13px] leading-tight text-foreground">
        <span><HighlightText text={clip.content.substring(0, PREVIEW_CHAR_LIMIT)} query={searchQuery} /></span>
      </pre>
    );
  }, [clip.clip_type, clip.content, clip.subtype, searchQuery]);

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
      e.dataTransfer.setData('text/plain', clip.content);
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
      role="option"
      aria-selected={isSelected}
      aria-label={`${title} clip: ${clip.preview?.substring(0, 50) || clip.clip_type}. ${clip.is_sensitive ? 'Sensitive content.' : ''}`}
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
        onClick={(e) => onSelect(e)}
        onDoubleClick={onPaste}
        onContextMenu={handleContextMenu}
        className={clsx(
          'relative flex h-full w-full cursor-pointer select-none flex-col overflow-hidden rounded-xl border bg-card',
          'transition-all duration-200 ease-out',
          isMultiSelected
            ? 'border-blue-500/70 ring-2 ring-blue-500/40'
            : isSelected
              ? 'z-10 scale-[1.04] -translate-y-1.5 border-blue-500/80 ring-[3px] ring-blue-500/40'
              : 'border-white/[0.08] dark:border-white/[0.08] hover:scale-[1.02] hover:-translate-y-[3px] hover:border-white/[0.16] dark:hover:border-white/[0.16]',
          'group'
        )}
      >
        {/* Multi-select order badge */}
        {isMultiSelected && multiSelectIndex != null && (
          <div className="absolute left-1.5 top-1.5 z-20 flex h-5 w-5 items-center justify-center rounded-full bg-blue-500 text-[10px] font-bold text-white shadow-md">
            {multiSelectIndex + 1}
          </div>
        )}

        {/* Header */}
        <div className={clsx(headerColor, 'flex flex-shrink-0 items-center gap-1.5 border-b border-black/10 px-2.5 py-2 dark:border-black/20')}>
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
          {clip.is_sensitive && (
            <span className="flex items-center gap-0.5 rounded bg-red-500/20 px-1 py-0.5 text-[9px] font-bold uppercase tracking-wider text-red-400">
              <ShieldAlert size={9} />
            </span>
          )}
          {subtypeBadge}
          {showPin && (
            <button
              onClick={(e) => {
                e.stopPropagation();
                onPin();
              }}
              className={clsx(
                'rounded-md p-1 transition-opacity duration-150 hover:bg-black/10',
                clip.is_pinned ? 'opacity-100' : 'opacity-40 group-hover:opacity-100'
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
              if (copiedTimerRef.current) clearTimeout(copiedTimerRef.current);
              copiedTimerRef.current = setTimeout(() => setCopied(false), 2000);
            }}
            className="rounded-md p-1 opacity-0 transition-opacity duration-150 hover:bg-black/10 group-hover:opacity-100"
            title="Copy to clipboard"
          >
            {copied ? (
              <Check size={14} className="animate-copy-pulse text-emerald-500" />
            ) : (
              <Copy size={14} className="text-foreground/70 hover:text-foreground" />
            )}
          </button>
        </div>

        {/* Content */}
        <div className={clsx('relative flex-1 overflow-hidden bg-card p-2', isMultiSelected && 'opacity-75', clip.is_sensitive && 'sensitive-blur')}>
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
        <div className="flex items-center justify-between bg-gradient-to-t from-black/[0.04] to-transparent px-2.5 py-1 dark:from-black/[0.15]">
          <span className="flex items-center gap-1.5 text-[10px] font-medium text-muted-foreground/40">
            <span>
              {clip.clip_type === 'image'
                ? (getImageSizeFromMeta(clip.metadata) ?? 'Image')
                : `${clip.content.length} chars`}
            </span>
            <span className="text-muted-foreground/25">·</span>
            <span title={clip.created_at}>{relativeTime(clip.created_at)}</span>
          </span>
          <span className="flex items-center gap-2 text-[10px] text-muted-foreground/35">
            {folderName && (
              <span className="flex items-center gap-0.5 rounded bg-indigo-500/15 px-1.5 py-0.5 text-[9px] font-semibold text-indigo-400" title={`In folder: ${folderName}`}>
                <Folder size={9} className="flex-shrink-0" />
                {folderName}
              </span>
            )}
            {clip.paste_count > 0 && (
              <span className="tabular-nums" title="Times pasted">×{clip.paste_count}</span>
            )}
          </span>
        </div>

      </div>

    </div>
  );
});
