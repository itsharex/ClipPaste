import { useEffect, useMemo, useRef, useState } from 'react';
import { ClipboardItem, FolderItem } from '../../types';
import {
  Trash2,
  Plus,
  Folder as FolderIcon,
  Pencil,
  ChevronRight,
  FileText,
  Image as ImageIcon,
  Link2,
  Code,
  File as FileIcon,
  Type,
  ArrowRightLeft,
  Search,
  Inbox,
  Loader2,
} from 'lucide-react';
import { invoke } from '@tauri-apps/api/core';
import { toast } from 'sonner';

interface FoldersTabProps {
  folders: FolderItem[];
  newFolderName: string;
  setNewFolderName: (v: string) => void;
  editingFolderId: string | null;
  setEditingFolderId: (v: string | null) => void;
  renameValue: string;
  setRenameValue: (v: string) => void;
  loadFolders: () => Promise<void>;
}

function formatRelativeTime(iso: string): string {
  const then = new Date(iso).getTime();
  if (Number.isNaN(then)) return '';
  const diff = Math.max(0, Math.floor((Date.now() - then) / 1000));
  if (diff < 60) return `${diff}s`;
  if (diff < 3600) return `${Math.floor(diff / 60)}m`;
  if (diff < 86400) return `${Math.floor(diff / 3600)}h`;
  if (diff < 604800) return `${Math.floor(diff / 86400)}d`;
  if (diff < 2592000) return `${Math.floor(diff / 604800)}w`;
  return `${Math.floor(diff / 2592000)}mo`;
}

function ClipTypeIcon({ type, className }: { type: string; className?: string }) {
  const props = { size: 14, className: className ?? 'text-muted-foreground shrink-0' };
  switch (type) {
    case 'image':
      return <ImageIcon {...props} />;
    case 'url':
      return <Link2 {...props} />;
    case 'html':
      return <Code {...props} />;
    case 'rtf':
      return <Type {...props} />;
    case 'file':
      return <FileIcon {...props} />;
    default:
      return <FileText {...props} />;
  }
}

export function FoldersTab({
  folders,
  newFolderName,
  setNewFolderName,
  editingFolderId,
  setEditingFolderId,
  renameValue,
  setRenameValue,
  loadFolders,
}: FoldersTabProps) {
  const [expandedId, setExpandedId] = useState<string | null>(null);
  const [clipsByFolder, setClipsByFolder] = useState<Record<string, ClipboardItem[]>>({});
  const [loadingId, setLoadingId] = useState<string | null>(null);
  const [moveTargetClipId, setMoveTargetClipId] = useState<string | null>(null);
  const [moveSearch, setMoveSearch] = useState('');
  const movePopoverRef = useRef<HTMLDivElement | null>(null);

  // Close move popover on outside click
  useEffect(() => {
    if (!moveTargetClipId) return;
    const onDown = (e: MouseEvent) => {
      if (movePopoverRef.current && !movePopoverRef.current.contains(e.target as Node)) {
        setMoveTargetClipId(null);
        setMoveSearch('');
      }
    };
    document.addEventListener('mousedown', onDown);
    return () => document.removeEventListener('mousedown', onDown);
  }, [moveTargetClipId]);

  const customFolders = useMemo(() => folders.filter((f) => !f.is_system), [folders]);

  const loadClipsForFolder = async (folderId: string) => {
    setLoadingId(folderId);
    try {
      const clips = await invoke<ClipboardItem[]>('get_clips', {
        filterId: folderId,
        limit: 500,
        offset: 0,
        previewOnly: true,
      });
      setClipsByFolder((prev) => ({ ...prev, [folderId]: clips }));
    } catch (e) {
      toast.error(`Failed to load clips: ${e}`);
    } finally {
      setLoadingId((cur) => (cur === folderId ? null : cur));
    }
  };

  const toggleExpand = async (folderId: string) => {
    if (expandedId === folderId) {
      setExpandedId(null);
      return;
    }
    setExpandedId(folderId);
    if (!clipsByFolder[folderId]) {
      await loadClipsForFolder(folderId);
    }
  };

  const handleCreateFolder = async () => {
    if (!newFolderName.trim()) return;
    try {
      await invoke('create_folder', { name: newFolderName.trim(), icon: null, color: null });
      setNewFolderName('');
      await loadFolders();
      toast.success('Folder created');
    } catch (e) {
      toast.error(`Failed to create folder: ${e}`);
    }
  };

  const handleDeleteFolder = async (id: string) => {
    try {
      await invoke('delete_folder', { id });
      if (expandedId === id) setExpandedId(null);
      setClipsByFolder((prev) => {
        const next = { ...prev };
        delete next[id];
        return next;
      });
      await loadFolders();
      toast.success('Folder deleted');
    } catch (e) {
      toast.error(`Failed to delete folder: ${e}`);
    }
  };

  const startRenameFolder = (folder: FolderItem) => {
    setEditingFolderId(folder.id);
    setRenameValue(folder.name);
  };

  const saveRenameFolder = async () => {
    if (!editingFolderId || !renameValue.trim()) return;
    try {
      await invoke('rename_folder', { id: editingFolderId, name: renameValue.trim() });
      setEditingFolderId(null);
      setRenameValue('');
      await loadFolders();
      toast.success('Folder renamed');
    } catch (e) {
      toast.error(`Failed to rename folder: ${e}`);
    }
  };

  const handleMoveClip = async (
    clipUuid: string,
    fromFolderId: string,
    toFolderId: string | null,
  ) => {
    try {
      await invoke('move_to_folder', { clipId: clipUuid, folderId: toFolderId });
      setMoveTargetClipId(null);
      setMoveSearch('');
      await loadFolders();
      await loadClipsForFolder(fromFolderId);
      if (toFolderId && clipsByFolder[toFolderId]) {
        await loadClipsForFolder(toFolderId);
      }
      toast.success(toFolderId ? 'Clip moved' : 'Clip moved to home');
    } catch (e) {
      toast.error(`Failed: ${e}`);
    }
  };

  const handleDeleteClip = async (clipUuid: string, folderId: string) => {
    try {
      await invoke('delete_clip', { id: clipUuid });
      await loadFolders();
      await loadClipsForFolder(folderId);
      toast.success('Clip deleted');
    } catch (e) {
      toast.error(`Failed: ${e}`);
    }
  };

  return (
    <section className="space-y-4">
      <div className="flex items-baseline justify-between">
        <h3 className="text-sm font-medium text-muted-foreground">Manage Folders</h3>
        {customFolders.length > 0 && (
          <span className="text-xs text-muted-foreground">
            {customFolders.length} folder{customFolders.length === 1 ? '' : 's'}
          </span>
        )}
      </div>

      <div className="flex gap-2">
        <input
          type="text"
          value={newFolderName}
          onChange={(e) => setNewFolderName(e.target.value)}
          placeholder="New folder name"
          className="flex-1 rounded-lg border border-border bg-input px-3 py-2 text-sm focus:outline-none focus:ring-2 focus:ring-ring"
          onKeyDown={(e) => e.key === 'Enter' && handleCreateFolder()}
        />
        <button
          onClick={handleCreateFolder}
          disabled={!newFolderName.trim()}
          className="btn btn-secondary px-3"
        >
          <Plus size={16} className="mr-1" />
          Add
        </button>
      </div>

      <div className="space-y-2">
        {customFolders.length === 0 ? (
          <p className="rounded-lg border border-dashed border-border py-6 text-center text-xs text-muted-foreground">
            No custom folders created.
          </p>
        ) : (
          customFolders.map((folder) => {
            const isExpanded = expandedId === folder.id;
            const isEditing = editingFolderId === folder.id;
            const clips = clipsByFolder[folder.id];
            const isLoading = loadingId === folder.id && !clips;

            return (
              <div
                key={folder.id}
                className="overflow-hidden rounded-lg border border-border bg-card transition-colors"
              >
                {/* Header row */}
                <div
                  className={`flex items-center gap-2 px-3 py-2.5 ${
                    isEditing ? '' : 'cursor-pointer hover:bg-accent/40'
                  } ${isExpanded ? 'bg-accent/30' : ''}`}
                  onClick={() => {
                    if (!isEditing) toggleExpand(folder.id);
                  }}
                >
                  {isEditing ? (
                    <div
                      className="flex flex-1 items-center gap-2"
                      onClick={(e) => e.stopPropagation()}
                    >
                      <FolderIcon size={16} className="shrink-0 text-blue-400" />
                      <input
                        type="text"
                        value={renameValue}
                        onChange={(e) => setRenameValue(e.target.value)}
                        className="flex-1 rounded-md border border-input bg-background px-2 py-1 text-sm focus:outline-none focus:ring-2 focus:ring-ring"
                        autoFocus
                        onKeyDown={(e) => {
                          if (e.key === 'Enter') saveRenameFolder();
                          if (e.key === 'Escape') setEditingFolderId(null);
                        }}
                      />
                      <button
                        onClick={saveRenameFolder}
                        className="rounded px-2 py-1 text-xs font-medium text-primary hover:bg-primary/10"
                      >
                        Save
                      </button>
                      <button
                        onClick={() => setEditingFolderId(null)}
                        className="rounded px-2 py-1 text-xs text-muted-foreground hover:bg-accent"
                      >
                        Cancel
                      </button>
                    </div>
                  ) : (
                    <>
                      <ChevronRight
                        size={14}
                        className={`shrink-0 text-muted-foreground transition-transform ${
                          isExpanded ? 'rotate-90' : ''
                        }`}
                      />
                      <FolderIcon size={16} className="shrink-0 text-blue-400" />
                      <span className="flex-1 truncate text-sm font-medium">{folder.name}</span>
                      <span className="shrink-0 rounded-full bg-muted px-2 py-0.5 text-[10px] font-medium tabular-nums text-muted-foreground">
                        {folder.item_count}
                      </span>
                      <div className="flex shrink-0 items-center gap-0.5">
                        <button
                          onClick={(e) => {
                            e.stopPropagation();
                            startRenameFolder(folder);
                          }}
                          className="rounded p-1.5 text-muted-foreground hover:bg-accent hover:text-foreground"
                          title="Rename"
                        >
                          <Pencil size={13} />
                        </button>
                        <button
                          onClick={(e) => {
                            e.stopPropagation();
                            handleDeleteFolder(folder.id);
                          }}
                          className="rounded p-1.5 text-muted-foreground hover:bg-destructive/10 hover:text-destructive"
                          title="Delete folder"
                        >
                          <Trash2 size={13} />
                        </button>
                      </div>
                    </>
                  )}
                </div>

                {/* Expanded clip list */}
                {isExpanded && !isEditing && (
                  <div className="border-t border-border bg-background/40">
                    {isLoading ? (
                      <div className="flex items-center justify-center gap-2 py-6 text-xs text-muted-foreground">
                        <Loader2 size={14} className="animate-spin" />
                        Loading clips…
                      </div>
                    ) : !clips || clips.length === 0 ? (
                      <div className="flex flex-col items-center justify-center gap-1.5 py-6 text-xs text-muted-foreground">
                        <Inbox size={18} className="opacity-50" />
                        <span>No clips in this folder</span>
                      </div>
                    ) : (
                      <ul className="max-h-72 divide-y divide-border/50 overflow-y-auto">
                        {clips.map((clip) => {
                          const isMoveOpen = moveTargetClipId === clip.id;
                          const previewText =
                            clip.clip_type === 'image'
                              ? 'Image'
                              : clip.preview?.trim() || '(empty)';
                          return (
                            <li
                              key={clip.id}
                              className="group relative flex items-center gap-2 px-3 py-2 hover:bg-accent/30"
                            >
                              <ClipTypeIcon type={clip.clip_type} />
                              <div className="min-w-0 flex-1">
                                <div className="truncate text-xs text-foreground/90">
                                  {previewText}
                                </div>
                                {clip.note && (
                                  <div className="truncate text-[11px] italic text-muted-foreground">
                                    {clip.note}
                                  </div>
                                )}
                              </div>
                              <span className="shrink-0 text-[10px] tabular-nums text-muted-foreground">
                                {formatRelativeTime(clip.created_at)}
                              </span>
                              <div className="flex shrink-0 items-center gap-0.5 opacity-0 transition-opacity group-hover:opacity-100 focus-within:opacity-100">
                                <button
                                  onClick={(e) => {
                                    e.stopPropagation();
                                    setMoveTargetClipId(isMoveOpen ? null : clip.id);
                                    setMoveSearch('');
                                  }}
                                  className="rounded p-1 text-muted-foreground hover:bg-accent hover:text-foreground"
                                  title="Move to another folder"
                                >
                                  <ArrowRightLeft size={12} />
                                </button>
                                <button
                                  onClick={(e) => {
                                    e.stopPropagation();
                                    handleDeleteClip(clip.id, folder.id);
                                  }}
                                  className="rounded p-1 text-muted-foreground hover:bg-destructive/10 hover:text-destructive"
                                  title="Delete clip"
                                >
                                  <Trash2 size={12} />
                                </button>
                              </div>

                              {/* Move popover */}
                              {isMoveOpen && (
                                <div
                                  ref={movePopoverRef}
                                  className="absolute right-2 top-9 z-20 w-60 overflow-hidden rounded-lg border border-border bg-popover shadow-lg"
                                  onClick={(e) => e.stopPropagation()}
                                >
                                  <div className="flex items-center gap-2 border-b border-border px-2.5 py-2">
                                    <Search size={12} className="text-muted-foreground" />
                                    <input
                                      autoFocus
                                      type="text"
                                      value={moveSearch}
                                      onChange={(e) => setMoveSearch(e.target.value)}
                                      placeholder="Search folders…"
                                      className="flex-1 bg-transparent text-xs focus:outline-none"
                                    />
                                  </div>
                                  <div className="max-h-56 overflow-y-auto py-1">
                                    {/* Move to home */}
                                    {'home'.includes(moveSearch.toLowerCase()) && (
                                      <button
                                        onClick={() =>
                                          handleMoveClip(clip.id, folder.id, null)
                                        }
                                        className="flex w-full items-center gap-2 px-2.5 py-1.5 text-left text-xs hover:bg-accent"
                                      >
                                        <Inbox size={13} className="text-muted-foreground" />
                                        <span>Move to Home</span>
                                      </button>
                                    )}
                                    {customFolders
                                      .filter((f) => f.id !== folder.id)
                                      .filter((f) =>
                                        f.name.toLowerCase().includes(moveSearch.toLowerCase()),
                                      )
                                      .map((f) => (
                                        <button
                                          key={f.id}
                                          onClick={() =>
                                            handleMoveClip(clip.id, folder.id, f.id)
                                          }
                                          className="flex w-full items-center gap-2 px-2.5 py-1.5 text-left text-xs hover:bg-accent"
                                        >
                                          <FolderIcon size={13} className="text-blue-400" />
                                          <span className="flex-1 truncate">{f.name}</span>
                                          <span className="text-[10px] tabular-nums text-muted-foreground">
                                            {f.item_count}
                                          </span>
                                        </button>
                                      ))}
                                    {customFolders.filter(
                                      (f) =>
                                        f.id !== folder.id &&
                                        f.name.toLowerCase().includes(moveSearch.toLowerCase()),
                                    ).length === 0 &&
                                      !'home'.includes(moveSearch.toLowerCase()) && (
                                        <div className="px-2.5 py-3 text-center text-[11px] text-muted-foreground">
                                          No matching folder
                                        </div>
                                      )}
                                  </div>
                                </div>
                              )}
                            </li>
                          );
                        })}
                      </ul>
                    )}
                  </div>
                )}
              </div>
            );
          })
        )}
      </div>
    </section>
  );
}
