import { FolderItem } from '../../types';
import {
  Trash2,
  Plus,
  Folder as FolderIcon,
  MoreHorizontal,
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

  return (
    <section className="space-y-4">
      <h3 className="text-sm font-medium text-muted-foreground">Manage Folders</h3>

      <div className="flex gap-2">
        <input
          type="text"
          value={newFolderName}
          onChange={(e) => setNewFolderName(e.target.value)}
          placeholder="New Folder Name"
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

      <div className="mt-4 space-y-2">
        {folders.filter((f) => !f.is_system).length === 0 ? (
          <p className="rounded-lg border border-dashed border-border py-4 text-center text-xs text-muted-foreground">
            No custom folders created.
          </p>
        ) : (
          folders
            .filter((f) => !f.is_system)
            .map((folder) => (
              <div
                key={folder.id}
                className="flex items-center justify-between rounded-lg border border-border bg-card p-3"
              >
                {editingFolderId === folder.id ? (
                  <div className="flex flex-1 items-center gap-2">
                    <input
                      type="text"
                      value={renameValue}
                      onChange={(e) => setRenameValue(e.target.value)}
                      className="flex-1 rounded-md border border-input bg-background px-2 py-1 text-sm"
                      autoFocus
                      onKeyDown={(e) => {
                        if (e.key === 'Enter') saveRenameFolder();
                        if (e.key === 'Escape') setEditingFolderId(null);
                      }}
                    />
                    <button
                      onClick={saveRenameFolder}
                      className="text-xs text-primary hover:underline"
                    >
                      Save
                    </button>
                    <button
                      onClick={() => setEditingFolderId(null)}
                      className="text-xs text-muted-foreground hover:underline"
                    >
                      Cancel
                    </button>
                  </div>
                ) : (
                  <>
                    <div className="flex items-center gap-3">
                      <FolderIcon size={16} className="text-blue-400" />
                      <span className="text-sm font-medium">{folder.name}</span>
                      <span className="text-xs text-muted-foreground">
                        ({folder.item_count} items)
                      </span>
                    </div>
                    <div className="flex items-center gap-2">
                      <button
                        onClick={() => startRenameFolder(folder)}
                        className="rounded p-1 text-muted-foreground hover:bg-accent hover:text-foreground"
                        title="Rename"
                      >
                        <MoreHorizontal size={14} />
                      </button>
                      <button
                        onClick={() => handleDeleteFolder(folder.id)}
                        className="rounded p-1 text-muted-foreground hover:bg-destructive/10 hover:text-destructive"
                        title="Delete"
                      >
                        <Trash2 size={14} />
                      </button>
                    </div>
                  </>
                )}
              </div>
            ))
        )}
      </div>
    </section>
  );
}
