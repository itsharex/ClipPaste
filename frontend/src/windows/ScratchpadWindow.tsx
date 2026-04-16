import { useState, useEffect, useCallback, useRef } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { getCurrentWindow, PhysicalSize, PhysicalPosition } from '@tauri-apps/api/window';
import { currentMonitor } from '@tauri-apps/api/window';
import { ScratchpadItem } from '../types';
import { useTheme } from '../hooks/useTheme';
import { X, Plus, Trash2, StickyNote, Copy, Check, Pin, PinOff, ClipboardPaste, Pencil, ChevronLeft, Search } from 'lucide-react';
import { clsx } from 'clsx';

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
    background: `linear-gradient(135deg, rgba(${c.rgb},0.04), rgba(${c.rgb},0.01))`,
    borderLeft: `2px solid rgba(${c.rgb},0.25)`,
  };
}

const COLLAPSED_WIDTH = 14;
const COLLAPSED_HEIGHT = 80;
const EXPANDED_WIDTH = 300;
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

  // Global ESC handler
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
      }
    };
    document.addEventListener('keydown', handleKey);
    return () => document.removeEventListener('keydown', handleKey);
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

  // Filtered scratchpads
  const filtered = searchQuery.trim()
    ? scratchpads.filter((s) => {
        const q = searchQuery.toLowerCase();
        return s.title.toLowerCase().includes(q) || s.content.toLowerCase().includes(q);
      })
    : scratchpads;

  // ── Window positioning ──
  const appWindow = getCurrentWindow();

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

  // Mode changes trigger window position
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
      const item = await invoke<ScratchpadItem>('create_scratchpad', { title: '', content: '', fieldsJson: null });
      setScratchpads((prev) => [...prev, item]);
      setEditingId(item.id);
      setEditTitle(''); setEditContent('');
      setMode('edit');
    } catch (e) { console.error(e); }
  }, []);

  const handleDelete = useCallback(async (id: string) => {
    try {
      await invoke('delete_scratchpad', { id });
      setScratchpads((prev) => prev.filter((s) => s.id !== id));
      if (editingId === id) { setEditingId(null); setMode('list'); }
      if (pastingId === id) { setPastingId(null); setMode('list'); }
    } catch {}
  }, [editingId, pastingId]);

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
        const item = await invoke<ScratchpadItem>('create_scratchpad', { title, content, fieldsJson: null });
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
        style={{ background: 'linear-gradient(135deg, hsl(var(--primary) / 0.15), hsl(var(--primary) / 0.3))', borderLeft: '2px solid hsl(var(--primary) / 0.4)' }}>
        <StickyNote size={8} className="text-amber-400/70" />
      </div>
    );
  }

  // ── Centered modal (paste or edit) ──
  if ((mode === 'paste' && pastingId) || (mode === 'edit' && editingId)) {
    const isPaste = mode === 'paste';
    const item = scratchpads.find((s) => s.id === (isPaste ? pastingId : editingId));
    if (!item) { goBack(); return null; }

    return (
      <div className="flex h-full w-full flex-col overflow-hidden rounded-2xl border border-border/20 bg-background/95 text-foreground shadow-2xl backdrop-blur-xl">
        {/* Header */}
        <div className="flex items-center gap-2 border-b border-border/30 px-4 py-3" data-tauri-drag-region>
          <button onClick={goBack} className="rounded p-1 text-muted-foreground hover:bg-accent hover:text-foreground">
            <ChevronLeft size={16} />
          </button>
          {isPaste
            ? <ClipboardPaste size={16} className="text-primary" />
            : <Pencil size={16} className="text-amber-400" />
          }
          <span className="flex-1 truncate text-sm font-semibold text-foreground/90">
            {isPaste ? (item.title || 'Paste snippet') : 'Edit note'}
          </span>
          <button onClick={handleClose} className="rounded p-1 text-muted-foreground hover:bg-accent hover:text-foreground">
            <X size={16} />
          </button>
        </div>

        {/* Content */}
        <div className="flex-1 overflow-y-auto p-4">
          {isPaste ? (
            <textarea
              ref={pasteTextareaRef}
              value={pasteContent}
              onChange={(e) => setPasteContent(e.target.value)}
              onKeyDown={(e) => {
                if (e.key === 'Escape') goBack();
                if (e.key === 'Enter' && e.ctrlKey) doPaste();
              }}
              className="h-full w-full resize-none rounded-xl border border-border/30 bg-input/30 px-4 py-3 text-sm leading-relaxed text-foreground outline-none focus:border-primary/50"
            />
          ) : (
            <>
              <label className="mb-1 block text-[10px] font-semibold uppercase tracking-wider text-muted-foreground/60">Title</label>
              <input ref={titleRef} value={editTitle} onChange={(e) => setEditTitle(e.target.value)}
                onKeyDown={(e) => { if (e.key === 'Escape') goBack(); }}
                className="mb-3 w-full rounded-lg border border-border/30 bg-input/30 px-3 py-2 text-sm font-semibold text-foreground outline-none focus:border-primary/50"
                placeholder="Title..." />
              {/* Color picker */}
              <label className="mb-1 block text-[10px] font-semibold uppercase tracking-wider text-muted-foreground/60">Color</label>
              <div className="mb-3 flex items-center gap-1.5">
                <button onClick={() => setEditColor(null)}
                  className={clsx('rounded-full border-2 p-1', !editColor ? 'border-foreground/50' : 'border-transparent')}
                  title="No color">
                  <X size={10} className="text-muted-foreground/50" />
                </button>
                {NOTE_COLORS.map((c) => (
                  <button key={c.key} onClick={() => setEditColor(c.key)}
                    className={clsx('h-5 w-5 rounded-full border-2 transition-transform', c.dot,
                      editColor === c.key ? 'border-foreground/60 scale-110' : 'border-transparent hover:scale-110'
                    )} title={c.key} />
                ))}
              </div>
              <label className="mb-1 block text-[10px] font-semibold uppercase tracking-wider text-muted-foreground/60">Content</label>
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
          <span className="text-[10px] text-muted-foreground/50">
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
      className="flex h-full w-full flex-col overflow-hidden text-foreground"
      style={{
        borderRadius: '14px 0 0 14px',
        background: `
          radial-gradient(ellipse at 15% 0%, rgba(139,92,246,0.045) 0%, transparent 45%),
          radial-gradient(ellipse at 85% 15%, rgba(59,130,246,0.035) 0%, transparent 45%),
          radial-gradient(ellipse at 35% 85%, rgba(236,72,153,0.03) 0%, transparent 45%),
          radial-gradient(ellipse at 90% 90%, rgba(45,212,191,0.025) 0%, transparent 40%),
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
            <StickyNote size={13} className="text-amber-400 drop-shadow-sm" />
            <span className="text-[11px] font-bold tracking-wide text-foreground/75">Scratchpad</span>
            <span className="rounded-full bg-white/[0.06] px-1.5 py-0.5 text-[9px] font-medium text-muted-foreground/60">{scratchpads.length}</span>
          </div>
          <div className="flex items-center gap-0.5">
            <button onClick={(e) => { e.stopPropagation(); setPinned(!pinned); }}
              className={clsx('rounded-md p-1 transition-all', pinned ? 'bg-amber-400/10 text-amber-400' : 'text-muted-foreground/25 hover:bg-white/[0.05] hover:text-muted-foreground/50')}
              title={pinned ? 'Unpin' : 'Pin open'}>
              {pinned ? <Pin size={11} /> : <PinOff size={11} />}
            </button>
            <button onClick={(e) => { e.stopPropagation(); handleAdd(); }}
              className="rounded-md p-1 text-emerald-400/60 transition-all hover:bg-emerald-400/10 hover:text-emerald-400" title="New note">
              <Plus size={12} />
            </button>
            <button onClick={(e) => { e.stopPropagation(); handleClose(); }}
              className="rounded-md p-1 text-muted-foreground/25 transition-all hover:bg-white/[0.05] hover:text-muted-foreground/50" title="Close">
              <X size={12} />
            </button>
          </div>
        </div>
      </div>

      {/* Search */}
      <div className="px-3 py-1.5">
        <div className="flex items-center gap-1.5 rounded-lg bg-white/[0.04] px-2.5 py-1.5 ring-1 ring-white/[0.05] transition-all focus-within:ring-primary/25">
          <Search size={10} className="text-muted-foreground/25" />
          <input ref={searchRef} value={searchQuery} onChange={(e) => setSearchQuery(e.target.value)}
            placeholder="Search..."
            className="flex-1 border-none bg-transparent text-[11px] text-foreground outline-none placeholder:text-muted-foreground/25" />
          {searchQuery && (
            <button onClick={() => setSearchQuery('')} className="text-muted-foreground/30 hover:text-foreground"><X size={9} /></button>
          )}
        </div>
      </div>

      {/* Notes */}
      <div className="no-scrollbar flex-1 overflow-y-auto px-2.5 pb-2">
        {filtered.length === 0 && !isDragOver && (
          <div className="flex flex-col items-center justify-center gap-3 py-16 text-center">
            <div className="rounded-2xl bg-white/[0.03] p-4"><StickyNote size={22} className="text-muted-foreground/12" /></div>
            <p className="text-[10px] text-muted-foreground/25">{searchQuery ? 'No matching notes' : 'Drag clips here or click +'}</p>
          </div>
        )}
        {isDragOver && filtered.length === 0 && (
          <div className="m-2 flex items-center justify-center rounded-xl border-2 border-dashed border-primary/25 bg-primary/[0.03] p-10">
            <span className="text-[11px] font-medium text-primary/40">Drop clip here</span>
          </div>
        )}

        {filtered.map((item, index) => {
          const colorStyle = getNoteColorStyle(item.color);
          const hasColor = !!item.color;
          return (
            <div key={item.id}
              draggable
              onDragStart={(e) => {
                const text = item.title ? `${item.title}\n${item.content}` : item.content;
                e.dataTransfer.setData('text/plain', text);
                e.dataTransfer.effectAllowed = 'copyMove';
                handleItemDragStart(item.id);
              }}
              onDragOver={(e) => handleItemDragOver(e, index)}
              onDrop={() => handleItemDrop(index)}
              onDragEnd={handleItemDragEnd}
              className={clsx(
                'group relative mb-2 flex overflow-hidden rounded-xl border transition-all duration-200 ease-out',
                'border-white/[0.06]',
                'hover:border-primary/30 hover:scale-[1.02] hover:shadow-lg hover:shadow-primary/5',
                dragOverIndex === index && dragItemRef.current && 'ring-1 ring-primary/30',
              )}
              style={{
                ...colorStyle,
                ...(!hasColor ? { background: 'hsl(var(--card) / 0.4)' } : {}),
              }}
            >
              {/* Paste strip — left */}
              <button
                onClick={(e) => { e.stopPropagation(); startPaste(item); }}
                className="flex w-8 flex-shrink-0 items-center justify-center border-r border-white/[0.03] text-foreground/10 transition-all hover:bg-white/[0.04] hover:text-primary/50"
                title="Paste"
              >
                <ClipboardPaste size={12} />
              </button>

              {/* Content */}
              <div className="min-w-0 flex-1 px-2.5 py-2">
                <div className="mb-0.5 flex items-center justify-between">
                  <div className="flex items-center gap-1 overflow-hidden">
                    {item.is_pinned && <Pin size={8} className="flex-shrink-0 text-amber-400/50" />}
                    {item.title ? (
                      <span className="truncate text-[11px] font-semibold text-foreground/80">{item.title}</span>
                    ) : (
                      <span className="text-[10px] text-muted-foreground/20">Untitled</span>
                    )}
                  </div>
                  {/* Hover toolbar */}
                  <div className="flex flex-shrink-0 items-center rounded-md bg-background/50 opacity-0 shadow-sm backdrop-blur-sm transition-all group-hover:opacity-100">
                    <button onClick={(e) => { e.stopPropagation(); handleToggleNotePin(item.id); }}
                      className={clsx('rounded-l-md p-1 transition-colors', item.is_pinned ? 'text-amber-400' : 'text-muted-foreground/30 hover:text-amber-400')}
                      title={item.is_pinned ? 'Unpin' : 'Pin'}><Pin size={9} /></button>
                    <button onClick={(e) => { e.stopPropagation(); startEdit(item); }}
                      className="p-1 text-muted-foreground/30 transition-colors hover:text-foreground/60" title="Edit"><Pencil size={9} /></button>
                    <button onClick={(e) => { e.stopPropagation(); handleCopyText(item.title ? `${item.title}\n${item.content}` : item.content, item.id); }}
                      className="p-1 text-muted-foreground/30 transition-colors hover:text-foreground/60" title="Copy">
                      {copiedId === item.id ? <Check size={9} className="text-emerald-400" /> : <Copy size={9} />}
                    </button>
                    <button onClick={(e) => { e.stopPropagation(); handleDelete(item.id); }}
                      className="rounded-r-md p-1 text-muted-foreground/30 transition-colors hover:text-red-400" title="Delete"><Trash2 size={9} /></button>
                  </div>
                </div>
                {item.content ? (
                  <p className="line-clamp-2 whitespace-pre-wrap break-words text-[10.5px] leading-relaxed text-foreground/45">{item.content}</p>
                ) : null}
              </div>
            </div>
          );
        })}

        {isDragOver && filtered.length > 0 && (
          <div className="mt-1 flex items-center justify-center rounded-xl border-2 border-dashed border-primary/15 bg-primary/[0.02] p-4">
            <span className="text-[10px] font-medium text-primary/30">Drop to add</span>
          </div>
        )}
      </div>
    </div>
  );
}
