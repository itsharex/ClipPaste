import { useState, useEffect, useCallback, useRef, useMemo } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { getCurrentWindow, PhysicalSize, PhysicalPosition } from '@tauri-apps/api/window';
import { currentMonitor } from '@tauri-apps/api/window';
import { listen } from '@tauri-apps/api/event';
import { ScratchpadItem } from '../types';
import { useTheme } from '../hooks/useTheme';
import { X, Plus, Trash2, StickyNote, Copy, Check, Pin, PinOff, ClipboardPaste, Pencil, ChevronLeft, Search, ArrowUpDown } from 'lucide-react';
import { clsx } from 'clsx';
import { Toaster, toast } from 'sonner';

type SortMode = 'manual' | 'alpha' | 'recent';

/**
 * Render simple markdown inline: **bold**, *italic*, `code`, and leading "- " or "* " bullets.
 * Deliberately minimal — no full parser, no links or headings.
 */
function renderInlineMarkdown(text: string): React.ReactNode[] {
  const out: React.ReactNode[] = [];
  const pattern = /(\*\*[^*\n]+\*\*|\*[^*\n]+\*|`[^`\n]+`)/g;
  let match: RegExpExecArray | null;
  let last = 0;
  let key = 0;
  while ((match = pattern.exec(text)) !== null) {
    if (match.index > last) out.push(text.slice(last, match.index));
    const tok = match[0];
    if (tok.startsWith('**') && tok.endsWith('**')) {
      out.push(<strong key={`b${key++}`} className="font-semibold text-foreground/90">{tok.slice(2, -2)}</strong>);
    } else if (tok.startsWith('`') && tok.endsWith('`')) {
      out.push(<code key={`c${key++}`} className="rounded bg-white/10 px-1 font-mono text-[10px]">{tok.slice(1, -1)}</code>);
    } else {
      out.push(<em key={`i${key++}`} className="italic">{tok.slice(1, -1)}</em>);
    }
    last = match.index + tok.length;
  }
  if (last < text.length) out.push(text.slice(last));
  return out.length > 0 ? out : [text];
}

function renderMarkdownPreview(content: string): React.ReactNode {
  // Split by newlines, apply bullet formatting per line
  const lines = content.split('\n');
  return lines.map((line, idx) => {
    const bullet = /^\s*[-*]\s+/.exec(line);
    if (bullet) {
      const rest = line.slice(bullet[0].length);
      return (
        <span key={idx} className="block">
          <span className="text-primary/70">• </span>
          {renderInlineMarkdown(rest)}
        </span>
      );
    }
    return <span key={idx} className="block">{renderInlineMarkdown(line)}</span>;
  });
}

const NOTE_COLORS: { key: string; dot: string; rgb: string }[] = [
  { key: 'red', dot: 'bg-red-400', rgb: '248,113,113' },
  { key: 'orange', dot: 'bg-orange-400', rgb: '251,146,60' },
  { key: 'amber', dot: 'bg-amber-400', rgb: '251,191,36' },
  { key: 'green', dot: 'bg-green-400', rgb: '74,222,128' },
  { key: 'teal', dot: 'bg-teal-400', rgb: '45,212,191' },
  { key: 'blue', dot: 'bg-blue-400', rgb: '96,165,250' },
  { key: 'violet', dot: 'bg-violet-400', rgb: '167,139,250' },
  { key: 'pink', dot: 'bg-pink-400', rgb: '244,114,182' },
];

function getNoteColorStyle(color: string | null): React.CSSProperties {
  if (!color) return {};
  const c = NOTE_COLORS.find((n) => n.key === color);
  if (!c) return {};
  return {
    background: `linear-gradient(135deg, rgba(${c.rgb},0.12), rgba(${c.rgb},0.04))`,
    borderLeft: `5px solid rgba(${c.rgb},0.7)`,
  };
}

const COLLAPSED_WIDTH = 16;
const COLLAPSED_HEIGHT = 100;
const EXPANDED_WIDTH = 320;
const MODAL_WIDTH = 520;
const MODAL_HEIGHT = 420;

type ViewMode = 'collapsed' | 'list' | 'paste' | 'edit';

export function ScratchpadWindow() {
  const [scratchpads, setScratchpads] = useState<ScratchpadItem[]>([]);
  const [mode, setMode] = useState<ViewMode>('collapsed');
  const [pinned, setPinned] = useState(false);
  const [searchQuery, setSearchQuery] = useState('');

  // Edit state
  const [editingId, setEditingId] = useState<string | null>(null);
  const [editTitle, setEditTitle] = useState('');
  const [editContent, setEditContent] = useState('');
  const [editColor, setEditColor] = useState<string | null>(null);

  // Paste state
  const [pastingId, setPastingId] = useState<string | null>(null);
  const [pasteContent, setPasteContent] = useState('');

  const [copiedId, setCopiedId] = useState<string | null>(null);
  const [dragOverIndex, setDragOverIndex] = useState<number | null>(null);
  const [isDragOver, setIsDragOver] = useState(false);
  // Keyboard selection, color filter, sort, sort menu visibility
  const [selectedId, setSelectedId] = useState<string | null>(null);
  const [colorFilter, setColorFilter] = useState<string | null>(null);
  const [sortMode, setSortMode] = useState<SortMode>('manual');
  const [showSortMenu, setShowSortMenu] = useState(false);
  const titleRef = useRef<HTMLInputElement>(null);
  const pasteTextareaRef = useRef<HTMLTextAreaElement>(null);
  const searchRef = useRef<HTMLInputElement>(null);
  const dragItemRef = useRef<string | null>(null);
  const panelRef = useRef<HTMLDivElement>(null);
  const collapseTimerRef = useRef<ReturnType<typeof setTimeout> | null>(null);
  const isResizingRef = useRef(false);

  // Disable right-click
  useEffect(() => {
    const prevent = (e: MouseEvent) => e.preventDefault();
    document.addEventListener('contextmenu', prevent);
    return () => document.removeEventListener('contextmenu', prevent);
  }, []);

  // Keep refs to values consumed inside the keydown handler — lets us declare the
  // handler once without forward-referencing hooks defined further down.
  const filteredRef = useRef<ScratchpadItem[]>([]);
  const selectedIdRef = useRef<string | null>(null);
  selectedIdRef.current = selectedId;

  // Keyboard handler: ESC, arrows, Enter/E/Del/"/" in list mode.
  useEffect(() => {
    const handleKey = (e: KeyboardEvent) => {
      if (e.key === 'Escape') {
        if (mode === 'paste' || mode === 'edit') {
          goBack();
        } else if (mode === 'list') {
          if (searchQuery) {
            setSearchQuery('');
          } else {
            handleClose();
          }
        }
        return;
      }

      if (mode !== 'list') return;
      const target = e.target as HTMLElement | null;
      const inInput = target && (target.tagName === 'INPUT' || target.tagName === 'TEXTAREA');

      if (e.key === '/' && !inInput) {
        e.preventDefault();
        searchRef.current?.focus();
        return;
      }

      if (inInput) return;

      const list = filteredRef.current;
      const curSelected = selectedIdRef.current;

      if (e.key === 'ArrowDown' || e.key === 'ArrowUp') {
        if (list.length === 0) return;
        e.preventDefault();
        const dir = e.key === 'ArrowDown' ? 1 : -1;
        const idx = curSelected ? list.findIndex((s) => s.id === curSelected) : -1;
        const next = idx < 0
          ? (dir === 1 ? 0 : list.length - 1)
          : Math.max(0, Math.min(list.length - 1, idx + dir));
        setSelectedId(list[next].id);
      } else if (e.key === 'Enter' && curSelected) {
        const item = list.find((s) => s.id === curSelected);
        if (item) startPaste(item);
      } else if ((e.key === 'e' || e.key === 'E') && curSelected && !e.ctrlKey && !e.metaKey) {
        const item = list.find((s) => s.id === curSelected);
        if (item) startEdit(item);
      } else if ((e.key === 'Delete' || e.key === 'Backspace') && curSelected) {
        handleDelete(curSelected);
      }
    };
    document.addEventListener('keydown', handleKey);
    return () => document.removeEventListener('keydown', handleKey);
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [mode, searchQuery]);

  // Theme
  const [themeSetting, setThemeSetting] = useState('system');
  useEffect(() => {
    invoke<Record<string, string>>('get_settings').then((s) => {
      if (s.theme) setThemeSetting(s.theme);
    }).catch(() => {});
  }, []);
  useTheme(themeSetting);

  // Load
  const loadScratchpads = useCallback(async () => {
    try { setScratchpads(await invoke<ScratchpadItem[]>('get_scratchpads')); }
    catch (e) { console.error('Failed to load scratchpads:', e); }
  }, []);
  useEffect(() => { loadScratchpads(); }, [loadScratchpads]);

  // Global hotkey listener — toggle between collapsed and list mode.
  useEffect(() => {
    const unlistenP = listen('scratchpad-toggle', () => {
      setMode((m) => (m === 'collapsed' ? 'list' : 'collapsed'));
      setPinned((p) => (mode === 'collapsed' ? true : p));
    });
    return () => { unlistenP.then((fn) => fn()).catch(() => {}); };
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  // Filter by search + color, then sort. Synced into filteredRef for keydown handler.
  const filtered = useMemo(() => {
    const q = searchQuery.trim().toLowerCase();
    let list = scratchpads.filter((s) => {
      if (colorFilter && s.color !== colorFilter) return false;
      if (q && !s.title.toLowerCase().includes(q) && !s.content.toLowerCase().includes(q)) return false;
      return true;
    });
    if (sortMode === 'alpha') {
      list = [...list].sort((a, b) => {
        // Pinned still go first even in sorted modes.
        if (a.is_pinned !== b.is_pinned) return a.is_pinned ? -1 : 1;
        return (a.title || a.content).localeCompare(b.title || b.content);
      });
    } else if (sortMode === 'recent') {
      list = [...list].sort((a, b) => {
        if (a.is_pinned !== b.is_pinned) return a.is_pinned ? -1 : 1;
        const ta = a.updated_at || a.created_at;
        const tb = b.updated_at || b.created_at;
        return tb.localeCompare(ta);
      });
    }
    return list;
  }, [scratchpads, searchQuery, colorFilter, sortMode]);
  filteredRef.current = filtered;

  // ── Window positioning ──
  // Memoize the window handle — getCurrentWindow() returns a fresh proxy on each call,
  // which made the moveTo* callbacks unstable and caused the position useEffect to re-run
  // on every render, producing a hide→resize→show flicker during typing.
  const appWindow = useMemo(() => getCurrentWindow(), []);

  const moveToSide = useCallback(async () => {
    isResizingRef.current = true;
    try {
      const scale = await appWindow.scaleFactor();
      const monitor = await currentMonitor();
      if (!monitor) return;
      const { width: workW, height: workH } = monitor.size;
      const { x: workX, y: workY } = monitor.position;
      const w = Math.round(EXPANDED_WIDTH * scale);
      const h = Math.round(workH * 0.75);
      await appWindow.setSize(new PhysicalSize(w, h));
      await appWindow.setPosition(new PhysicalPosition(workX + workW - w, workY + Math.round((workH - h) / 2)));
    } catch {} finally { setTimeout(() => { isResizingRef.current = false; }, 200); }
  }, [appWindow]);

  const moveToCenter = useCallback(async () => {
    isResizingRef.current = true;
    try {
      await appWindow.hide();
      const scale = await appWindow.scaleFactor();
      const monitor = await currentMonitor();
      if (!monitor) return;
      const { width: workW, height: workH } = monitor.size;
      const { x: workX, y: workY } = monitor.position;
      const w = Math.round(MODAL_WIDTH * scale);
      const h = Math.round(MODAL_HEIGHT * scale);
      await appWindow.setSize(new PhysicalSize(w, h));
      await appWindow.setPosition(new PhysicalPosition(
        workX + Math.round((workW - w) / 2), workY + Math.round((workH - h) / 2),
      ));
      await appWindow.show();
      await appWindow.setFocus();
    } catch {} finally { setTimeout(() => { isResizingRef.current = false; }, 200); }
  }, [appWindow]);

  const moveToCollapsed = useCallback(async () => {
    isResizingRef.current = true;
    try {
      const scale = await appWindow.scaleFactor();
      const monitor = await currentMonitor();
      if (!monitor) return;
      const { width: workW, height: workH } = monitor.size;
      const { x: workX, y: workY } = monitor.position;
      const w = Math.round(COLLAPSED_WIDTH * scale);
      const h = Math.round(COLLAPSED_HEIGHT * scale);
      await appWindow.setSize(new PhysicalSize(w, h));
      await appWindow.setPosition(new PhysicalPosition(workX + workW - w, workY + Math.round((workH - h) / 2)));
    } catch {} finally { setTimeout(() => { isResizingRef.current = false; }, 200); }
  }, [appWindow]);

  // Mode changes trigger window position. paste/edit go to centered modal
  // for roomy editing; list/collapsed pin to the side.
  useEffect(() => {
    if (mode === 'collapsed') moveToCollapsed();
    else if (mode === 'list') moveToSide();
    else if (mode === 'paste' || mode === 'edit') moveToCenter();
  }, [mode, moveToCollapsed, moveToSide, moveToCenter]);

  useEffect(() => {
    if (mode === 'edit' && titleRef.current) titleRef.current.focus();
  }, [mode, editingId]);
  useEffect(() => {
    if (mode === 'paste' && pasteTextareaRef.current) {
      pasteTextareaRef.current.focus();
      const len = pasteTextareaRef.current.value.length;
      pasteTextareaRef.current.setSelectionRange(len, len);
    }
  }, [mode, pastingId]);

  // ── Hover logic ──
  const cancelCollapse = useCallback(() => {
    if (collapseTimerRef.current) { clearTimeout(collapseTimerRef.current); collapseTimerRef.current = null; }
  }, []);

  const handleMouseEnter = useCallback(() => {
    if (isResizingRef.current || mode !== 'collapsed') return;
    cancelCollapse();
    // Snapshot whichever app the user is currently in BEFORE we grab focus — paste later
    // routes Shift+Insert back to that HWND. Fire-and-forget.
    invoke('capture_prev_foreground').catch(() => {});
    setMode('list');
  }, [mode, cancelCollapse]);

  const handleMouseLeave = useCallback(() => {
    if (isResizingRef.current || pinned || mode !== 'list') return;
    cancelCollapse();
    collapseTimerRef.current = setTimeout(() => setMode('collapsed'), 600);
  }, [pinned, mode, cancelCollapse]);

  const goBack = useCallback(() => {
    setEditingId(null);
    setPastingId(null);
    setPinned(true);
    setMode('list');
  }, []);

  const handleClose = useCallback(() => {
    setPinned(false);
    setEditingId(null);
    setPastingId(null);
    setSearchQuery('');
    setMode('collapsed');
  }, []);

  const handlePanelClick = useCallback(() => {
    if (!pinned && mode === 'list') setPinned(true);
  }, [pinned, mode]);

  // ── Edit ──
  const startEdit = useCallback((item: ScratchpadItem) => {
    setPastingId(null);
    setEditingId(item.id);
    setEditTitle(item.title);
    setEditContent(item.content);
    setEditColor(item.color);
    setMode('edit');
  }, []);

  const saveEdit = useCallback(async () => {
    if (!editingId) return;
    const t = editTitle.trim(), c = editContent.trim();
    if (t || c) {
      await invoke('update_scratchpad', { id: editingId, title: t, content: c, color: editColor || '' });
      setScratchpads((prev) => prev.map((s) => s.id === editingId ? { ...s, title: t, content: c, color: editColor } : s));
    } else {
      await invoke('delete_scratchpad', { id: editingId });
      setScratchpads((prev) => prev.filter((s) => s.id !== editingId));
    }
    setEditingId(null);
    setPinned(true);
    setMode('list');
  }, [editingId, editTitle, editContent, editColor]);

  // ── Paste ──
  const startPaste = useCallback((item: ScratchpadItem) => {
    setEditingId(null);
    setPastingId(item.id);
    setPasteContent(item.title ? `${item.title}\n${item.content}` : item.content);
    setMode('paste');
  }, []);

  const doPaste = useCallback(async () => {
    if (!pastingId) return;
    // Hide window FIRST to avoid flicker
    await appWindow.hide();
    try {
      await invoke('scratchpad_paste', { text: pasteContent });
    } catch {
      await navigator.clipboard.writeText(pasteContent);
    }
    setPastingId(null);
    setPinned(false);
    // Restore collapsed tab after a delay
    setTimeout(async () => {
      setMode('collapsed');
      // moveToCollapsed will run from useEffect, then show
      setTimeout(() => { appWindow.show().catch(() => {}); }, 300);
    }, 200);
  }, [pastingId, pasteContent, appWindow]);

  // ── CRUD ──
  const handleAdd = useCallback(async () => {
    try {
      const item = await invoke<ScratchpadItem>('create_scratchpad', { title: '', content: '' });
      setScratchpads((prev) => [...prev, item]);
      setEditingId(item.id);
      setEditTitle(''); setEditContent('');
      setMode('edit');
    } catch (e) { console.error(e); }
  }, []);

  const handleDelete = useCallback(async (id: string) => {
    const victim = scratchpads.find((s) => s.id === id);
    if (!victim) return;
    try {
      await invoke('delete_scratchpad', { id });
      setScratchpads((prev) => prev.filter((s) => s.id !== id));
      if (editingId === id) { setEditingId(null); setMode('list'); }
      if (pastingId === id) { setPastingId(null); setMode('list'); }
      if (selectedId === id) setSelectedId(null);
      // Offer undo for 5s — recreates the note (new uuid/id, same content/title/color).
      toast(`Deleted "${victim.title || victim.content.slice(0, 40) || 'note'}"`, {
        duration: 5000,
        action: {
          label: 'Undo',
          onClick: async () => {
            try {
              const restored = await invoke<ScratchpadItem>('create_scratchpad', {
                title: victim.title,
                content: victim.content,
              });
              // Restore color in a second call (create_scratchpad doesn't take color).
              if (victim.color) {
                await invoke('update_scratchpad', { id: restored.id, color: victim.color });
                restored.color = victim.color;
              }
              setScratchpads((prev) => [...prev, restored]);
            } catch (e) {
              console.error('Undo delete failed:', e);
            }
          },
        },
      });
    } catch {}
  }, [scratchpads, editingId, pastingId, selectedId]);

  const handleToggleNotePin = useCallback(async (id: string) => {
    try {
      const newVal = await invoke<boolean>('toggle_scratchpad_pin', { id });
      setScratchpads((prev) => prev.map((s) => s.id === id ? { ...s, is_pinned: newVal } : s));
    } catch {}
  }, []);

  const handleCopyText = useCallback(async (text: string, id: string) => {
    try {
      await navigator.clipboard.writeText(text);
      setCopiedId(id); setTimeout(() => setCopiedId(null), 1500);
    } catch {}
  }, []);

  // ── Drag ──
  const handleDrop = useCallback(async (e: React.DragEvent) => {
    e.preventDefault(); e.stopPropagation(); setIsDragOver(false);
    const text = e.dataTransfer.getData('text/plain');
    if (text) {
      try {
        const lines = text.split('\n');
        const title = lines[0].slice(0, 80);
        const content = lines.length > 1 ? lines.slice(1).join('\n') : '';
        const item = await invoke<ScratchpadItem>('create_scratchpad', { title, content });
        setScratchpads((prev) => [...prev, item]);
        setPinned(true);
      } catch {}
    }
  }, []);
  const handleDragOver = useCallback((e: React.DragEvent) => {
    e.preventDefault(); e.dataTransfer.dropEffect = 'copy'; setIsDragOver(true);
    if (mode === 'collapsed') setMode('list');
  }, [mode]);
  const handleDragLeave = useCallback((e: React.DragEvent) => {
    if (panelRef.current && !panelRef.current.contains(e.relatedTarget as Node)) setIsDragOver(false);
  }, []);
  const handleItemDragStart = useCallback((id: string) => { dragItemRef.current = id; }, []);
  const handleItemDragOver = useCallback((e: React.DragEvent, i: number) => {
    e.preventDefault(); if (dragItemRef.current) setDragOverIndex(i);
  }, []);
  const handleItemDrop = useCallback(async (index: number) => {
    const dragId = dragItemRef.current;
    if (!dragId) return;
    const ids = scratchpads.map((s) => s.id);
    const di = ids.indexOf(dragId);
    if (di === -1 || di === index) { dragItemRef.current = null; setDragOverIndex(null); return; }
    const r = [...ids]; const [m] = r.splice(di, 1); r.splice(index, 0, m);
    const map = new Map(scratchpads.map((s) => [s.id, s]));
    setScratchpads(r.map((id) => map.get(id)!).filter(Boolean));
    await invoke('reorder_scratchpads', { ids: r }).catch(() => {});
    dragItemRef.current = null; setDragOverIndex(null);
  }, [scratchpads]);
  const handleItemDragEnd = useCallback(() => { dragItemRef.current = null; setDragOverIndex(null); }, []);

  // ═══════════════════════════════════
  //  RENDER
  // ═══════════════════════════════════

  // ── Collapsed ──
  if (mode === 'collapsed') {
    return (
      <div className="flex h-full w-full cursor-pointer items-center justify-center rounded-l-lg"
        onMouseEnter={handleMouseEnter} onDragOver={handleDragOver} onDrop={handleDrop}
        style={{
          background: 'linear-gradient(180deg, hsl(var(--primary) / 0.15), hsl(var(--primary) / 0.35), hsl(var(--primary) / 0.15))',
          borderLeft: '2px solid hsl(var(--primary) / 0.5)',
          boxShadow: 'inset 1px 0 0 hsl(var(--primary) / 0.25)',
        }}
        title="Scratchpad — hover to open">
        {/* Minimal grip — 3 dots centered vertically, subtle primary tint */}
        <div className="flex flex-col gap-1 opacity-60">
          <span className="block h-0.5 w-0.5 rounded-full bg-primary-foreground/80" />
          <span className="block h-0.5 w-0.5 rounded-full bg-primary-foreground/80" />
          <span className="block h-0.5 w-0.5 rounded-full bg-primary-foreground/80" />
        </div>
      </div>
    );
  }

  // ── Centered modal (paste or edit) — separate window size for roomy editing ──
  if ((mode === 'paste' && pastingId) || (mode === 'edit' && editingId)) {
    const isPaste = mode === 'paste';
    const item = scratchpads.find((s) => s.id === (isPaste ? pastingId : editingId));
    if (!item) { goBack(); return null; }

    return (
      <div className="flex h-full w-full flex-col overflow-hidden rounded-2xl border border-border/20 bg-background/95 text-foreground shadow-2xl backdrop-blur-xl">
        {/* Header */}
        <div className="flex items-center gap-2 border-b border-border/30 px-4 py-3" data-tauri-drag-region>
          <button onClick={goBack} className="rounded p-1 text-muted-foreground hover:bg-accent hover:text-foreground" title="Back">
            <ChevronLeft size={16} />
          </button>
          {isPaste
            ? <ClipboardPaste size={16} className="text-primary" />
            : <Pencil size={16} className="text-amber-400" />
          }
          <span className="flex-1 truncate text-sm font-semibold text-foreground/90">
            {isPaste ? (item.title || 'Paste snippet') : 'Edit note'}
          </span>
          <button onClick={handleClose} className="rounded p-1 text-muted-foreground hover:bg-accent hover:text-foreground" title="Close">
            <X size={16} />
          </button>
        </div>

        {/* Content */}
        <div className="flex flex-1 flex-col overflow-y-auto p-4">
          {isPaste ? (
            <textarea
              ref={pasteTextareaRef}
              value={pasteContent}
              onChange={(e) => setPasteContent(e.target.value)}
              onKeyDown={(e) => {
                if (e.key === 'Escape') goBack();
                if (e.key === 'Enter' && e.ctrlKey) doPaste();
              }}
              className="h-full w-full flex-1 resize-none rounded-xl border border-border/30 bg-input/30 px-4 py-3 text-sm leading-relaxed text-foreground outline-none focus:border-primary/50"
            />
          ) : (
            <>
              <label className="mb-1 block text-[10px] font-semibold uppercase tracking-wider text-muted-foreground/70">Title</label>
              <input ref={titleRef} value={editTitle} onChange={(e) => setEditTitle(e.target.value)}
                onKeyDown={(e) => { if (e.key === 'Escape') goBack(); }}
                className="mb-3 w-full rounded-lg border border-border/30 bg-input/30 px-3 py-2 text-sm font-semibold text-foreground outline-none focus:border-primary/50"
                placeholder="Title..." />
              <label className="mb-1 block text-[10px] font-semibold uppercase tracking-wider text-muted-foreground/70">Color</label>
              <div className="mb-3 flex items-center gap-1.5">
                <button onClick={() => setEditColor(null)}
                  className={clsx('rounded-full border-2 p-1', !editColor ? 'border-foreground/60' : 'border-transparent')}
                  title="No color">
                  <X size={10} className="text-muted-foreground/60" />
                </button>
                {NOTE_COLORS.map((c) => (
                  <button key={c.key} onClick={() => setEditColor(c.key)}
                    className={clsx('h-5 w-5 rounded-full border-2 transition-transform', c.dot,
                      editColor === c.key ? 'border-foreground/70 scale-110' : 'border-transparent hover:scale-110'
                    )} title={c.key} />
                ))}
              </div>
              <label className="mb-1 block text-[10px] font-semibold uppercase tracking-wider text-muted-foreground/70">Content</label>
              <textarea value={editContent} onChange={(e) => setEditContent(e.target.value)}
                onKeyDown={(e) => {
                  if (e.key === 'Escape') goBack();
                  if (e.key === 'Enter' && e.ctrlKey) saveEdit();
                }}
                className="w-full flex-1 resize-none rounded-xl border border-border/30 bg-input/30 px-3 py-2 text-sm leading-relaxed text-foreground outline-none focus:border-primary/50"
                rows={6} placeholder="Content..." />
            </>
          )}
        </div>

        {/* Actions */}
        <div className="flex items-center justify-between border-t border-border/30 px-4 py-3">
          <span className="text-[10px] text-muted-foreground/60">
            {isPaste ? 'Ctrl+Enter to paste · Esc to cancel' : 'Ctrl+Enter to save · Esc to cancel'}
          </span>
          {isPaste ? (
            <button onClick={doPaste}
              className="flex items-center gap-2 rounded-xl bg-primary px-6 py-2.5 text-sm font-bold text-primary-foreground transition-colors hover:bg-primary/90 active:bg-primary/80">
              <ClipboardPaste size={16} /> Paste
            </button>
          ) : (
            <div className="flex gap-2">
              <button onClick={goBack} className="rounded-lg px-4 py-2 text-xs text-muted-foreground hover:bg-accent">Cancel</button>
              <button onClick={saveEdit} className="rounded-lg bg-primary/20 px-4 py-2 text-xs font-semibold text-primary hover:bg-primary/30">Save</button>
            </div>
          )}
        </div>
      </div>
    );
  }

  // ── List view (side panel) — glassmorphism design ──
  return (
    <div ref={panelRef}
      className="relative flex h-full w-full flex-col overflow-hidden text-foreground"
      style={{
        borderRadius: '14px 0 0 14px',
        background: `
          radial-gradient(ellipse at 20% 10%, rgba(139,92,246,0.05) 0%, transparent 50%),
          radial-gradient(ellipse at 80% 90%, rgba(59,130,246,0.035) 0%, transparent 50%),
          linear-gradient(180deg, hsl(var(--background)), hsl(var(--background) / 0.97))
        `,
        borderLeft: '1px solid hsl(var(--border) / 0.08)',
        boxShadow: 'inset 0 1px 0 hsl(var(--border) / 0.06), inset -1px 0 0 hsl(var(--border) / 0.04)',
      }}
      onMouseEnter={handleMouseEnter} onMouseLeave={handleMouseLeave} onClick={handlePanelClick}
      onDragOver={handleDragOver} onDragLeave={handleDragLeave} onDrop={handleDrop}>

      {/* Header */}
      <div className="border-b border-white/[0.06] px-3 py-2.5">
        <div className="flex items-center justify-between">
          <div className="flex items-center gap-2">
            <StickyNote size={14} className="text-amber-400 drop-shadow-sm" />
            <span className="text-xs font-bold tracking-wide text-foreground/90">Scratchpad</span>
            <span className="rounded-full bg-white/[0.08] px-1.5 py-0.5 text-[10px] font-medium text-muted-foreground/80">{scratchpads.length}</span>
          </div>
          <div className="relative flex items-center gap-1">
            <button onClick={(e) => { e.stopPropagation(); setShowSortMenu((v) => !v); }}
              className={clsx('rounded-md p-1.5 transition-all', sortMode !== 'manual' ? 'bg-primary/20 text-primary' : 'text-muted-foreground/50 hover:bg-white/[0.08] hover:text-foreground/80')}
              title={`Sort: ${sortMode}`}>
              <ArrowUpDown size={13} />
            </button>
            {showSortMenu && (
              <div onClick={(e) => e.stopPropagation()}
                className="absolute right-24 top-7 z-30 w-28 overflow-hidden rounded-md border border-border/30 bg-background/95 py-1 text-xs shadow-xl backdrop-blur-md">
                {(['manual', 'alpha', 'recent'] as SortMode[]).map((m) => (
                  <button key={m}
                    onClick={() => { setSortMode(m); setShowSortMenu(false); }}
                    className={clsx('block w-full px-3 py-1.5 text-left capitalize transition-colors', sortMode === m ? 'bg-primary/15 text-primary' : 'text-foreground/80 hover:bg-white/[0.08]')}>
                    {m === 'alpha' ? 'A–Z' : m === 'recent' ? 'Recent' : 'Manual'}
                  </button>
                ))}
              </div>
            )}
            <button onClick={(e) => { e.stopPropagation(); setPinned(!pinned); }}
              className={clsx('rounded-md p-1.5 transition-all', pinned ? 'bg-amber-400/15 text-amber-400' : 'text-muted-foreground/50 hover:bg-white/[0.08] hover:text-foreground/80')}
              title={pinned ? 'Unpin' : 'Pin open'}>
              {pinned ? <Pin size={13} /> : <PinOff size={13} />}
            </button>
            <button onClick={(e) => { e.stopPropagation(); handleAdd(); }}
              className="rounded-md p-1.5 text-emerald-400/80 transition-all hover:bg-emerald-400/15 hover:text-emerald-400" title="New note">
              <Plus size={14} />
            </button>
            <button onClick={(e) => { e.stopPropagation(); handleClose(); }}
              className="rounded-md p-1.5 text-muted-foreground/50 transition-all hover:bg-white/[0.08] hover:text-foreground/80" title="Close">
              <X size={14} />
            </button>
          </div>
        </div>
      </div>

      {/* Color filter — shown when any notes have a color. Click dot to filter/clear. */}
      {scratchpads.some((s) => s.color) && (
        <div className="flex items-center gap-1 px-3 py-1">
          <button
            onClick={(e) => { e.stopPropagation(); setColorFilter(null); }}
            className={clsx('flex h-4 w-4 items-center justify-center rounded-full border', !colorFilter ? 'border-foreground/50 bg-white/10' : 'border-transparent text-muted-foreground/50 hover:bg-white/[0.08]')}
            title="All colors">
            <span className="text-[8px]">All</span>
          </button>
          {NOTE_COLORS.filter((c) => scratchpads.some((s) => s.color === c.key)).map((c) => (
            <button key={c.key}
              onClick={(e) => { e.stopPropagation(); setColorFilter(colorFilter === c.key ? null : c.key); }}
              className={clsx('h-4 w-4 rounded-full border-2 transition-all', c.dot,
                colorFilter === c.key ? 'border-foreground/70 scale-110' : 'border-transparent hover:scale-110')}
              title={c.key} />
          ))}
        </div>
      )}

      {/* Search */}
      <div className="px-3 py-1.5">
        <div className="flex items-center gap-1.5 rounded-lg bg-white/[0.05] px-2.5 py-1.5 ring-1 ring-white/[0.06] transition-all focus-within:ring-primary/30">
          <Search size={12} className="text-muted-foreground/50" />
          <input ref={searchRef} value={searchQuery} onChange={(e) => setSearchQuery(e.target.value)}
            placeholder="Search..."
            className="flex-1 border-none bg-transparent text-xs text-foreground outline-none placeholder:text-muted-foreground/40" />
          {searchQuery && (
            <button onClick={() => setSearchQuery('')} className="text-muted-foreground/50 hover:text-foreground"><X size={11} /></button>
          )}
        </div>
      </div>

      {/* Notes */}
      <div className="no-scrollbar flex-1 overflow-y-auto px-2.5 pb-2">
        {filtered.length === 0 && !isDragOver && (
          <div className="flex flex-col items-center justify-center gap-3 py-16 text-center">
            <div className="rounded-2xl bg-white/[0.05] p-4"><StickyNote size={24} className="text-muted-foreground/40" /></div>
            <p className="text-xs text-muted-foreground/60">{searchQuery ? 'No matching notes' : 'Drag clips here or click +'}</p>
          </div>
        )}
        {isDragOver && filtered.length === 0 && (
          <div className="m-2 flex items-center justify-center rounded-xl border-2 border-dashed border-primary/30 bg-primary/[0.05] p-10">
            <span className="text-xs font-medium text-primary/60">Drop clip here</span>
          </div>
        )}

        {filtered.map((item, index) => {
          const colorStyle = getNoteColorStyle(item.color);
          const hasColor = !!item.color;
          const isSelected = selectedId === item.id;
          const prev = index > 0 ? filtered[index - 1] : null;
          // Render a soft divider when we cross from pinned to unpinned (manual sort only — sorted modes already group correctly).
          const showPinnedDivider = prev && prev.is_pinned && !item.is_pinned;
          const charCount = item.content.length;
          return (
            <div key={item.id}>
              {showPinnedDivider && (
                <div className="my-2 flex items-center gap-2 px-1 text-[9px] font-semibold uppercase tracking-wider text-muted-foreground/40">
                  <div className="h-px flex-1 bg-white/[0.06]" />
                  <span>Others</span>
                  <div className="h-px flex-1 bg-white/[0.06]" />
                </div>
              )}
              <div
                draggable
                onClick={(e) => { e.stopPropagation(); setSelectedId(item.id); }}
                onDragStart={(e) => {
                  const text = item.title ? `${item.title}\n${item.content}` : item.content;
                  e.dataTransfer.setData('text/plain', text);
                  e.dataTransfer.effectAllowed = 'copyMove';
                  handleItemDragStart(item.id);
                }}
                onDragOver={(e) => handleItemDragOver(e, index)}
                onDrop={() => handleItemDrop(index)}
                onDragEnd={handleItemDragEnd}
                onDoubleClick={(e) => { e.stopPropagation(); startPaste(item); }}
                className={clsx(
                  'group relative mb-2 flex overflow-hidden rounded-xl border transition-all duration-200 ease-out',
                  item.is_pinned ? 'border-amber-400/30' : 'border-white/[0.08]',
                  isSelected ? 'ring-2 ring-primary/50' : 'hover:border-primary/40 hover:shadow-lg hover:shadow-primary/10',
                  dragOverIndex === index && dragItemRef.current && 'ring-1 ring-primary/30',
                )}
                style={{
                  ...colorStyle,
                  ...(!hasColor ? { background: 'hsl(var(--card) / 0.5)' } : {}),
                }}
              >
                {/* Left paste strip */}
                <button
                  onClick={(e) => { e.stopPropagation(); startPaste(item); }}
                  className="flex w-7 flex-shrink-0 items-center justify-center border-r border-white/[0.06] text-muted-foreground/60 transition-colors hover:bg-primary/15 hover:text-primary"
                  title="Paste"
                >
                  <ClipboardPaste size={12} />
                </button>

                {/* Content */}
                <div className="min-w-0 flex-1 px-2.5 py-2">
                  <div className="mb-1 flex items-center justify-between gap-2">
                    <div className="flex min-w-0 items-center gap-1.5 overflow-hidden">
                      {item.is_pinned && <Pin size={10} className="flex-shrink-0 text-amber-400" />}
                      {item.title ? (
                        <span className="truncate text-xs font-semibold text-foreground/95">{item.title}</span>
                      ) : (
                        <span className="text-[11px] italic text-muted-foreground/50">Untitled</span>
                      )}
                    </div>
                    <div className="flex flex-shrink-0 items-center gap-0.5 rounded-md bg-background/80 px-0.5 opacity-0 shadow-sm ring-1 ring-border/30 backdrop-blur-sm transition-all group-hover:opacity-100">
                      <button onClick={(e) => { e.stopPropagation(); handleToggleNotePin(item.id); }}
                        className={clsx('rounded p-1.5 transition-colors', item.is_pinned ? 'text-amber-400' : 'text-muted-foreground/70 hover:bg-amber-400/15 hover:text-amber-400')}
                        title={item.is_pinned ? 'Unpin' : 'Pin'}><Pin size={12} /></button>
                      <button onClick={(e) => { e.stopPropagation(); startEdit(item); }}
                        className="rounded p-1.5 text-muted-foreground/70 transition-colors hover:bg-amber-400/15 hover:text-amber-400" title="Edit"><Pencil size={12} /></button>
                      <button onClick={(e) => { e.stopPropagation(); handleCopyText(item.title ? `${item.title}\n${item.content}` : item.content, item.id); }}
                        className="rounded p-1.5 text-muted-foreground/70 transition-colors hover:bg-white/[0.08] hover:text-foreground/90" title="Copy">
                        {copiedId === item.id ? <Check size={12} className="text-emerald-400" /> : <Copy size={12} />}
                      </button>
                      <div className="mx-0.5 h-4 w-px bg-border/40" />
                      <button onClick={(e) => { e.stopPropagation(); handleDelete(item.id); }}
                        className="rounded p-1.5 text-muted-foreground/70 transition-colors hover:bg-red-400/15 hover:text-red-400" title="Delete"><Trash2 size={12} /></button>
                    </div>
                  </div>
                  {item.content ? (
                    <div className="line-clamp-2 whitespace-pre-wrap break-words text-[11px] leading-relaxed text-foreground/70">
                      {renderMarkdownPreview(item.content)}
                    </div>
                  ) : null}
                  {/* Char counter — subtle, shows on hover only */}
                  {charCount > 0 && (
                    <div className="mt-1 text-right text-[9px] font-medium text-muted-foreground/40 opacity-0 transition-opacity group-hover:opacity-100">
                      {charCount >= 1000 ? `${(charCount / 1000).toFixed(1)}k` : charCount} chars
                    </div>
                  )}
                </div>
              </div>
            </div>
          );
        })}

        {isDragOver && filtered.length > 0 && (
          <div className="mt-1 flex items-center justify-center rounded-xl border-2 border-dashed border-primary/20 bg-primary/[0.03] p-4">
            <span className="text-[11px] font-medium text-primary/50">Drop to add</span>
          </div>
        )}
      </div>
      <Toaster richColors position="bottom-center" theme="dark" toastOptions={{ style: { fontSize: '12px' } }} />
    </div>
  );
}
