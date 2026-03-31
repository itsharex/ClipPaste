import { Settings, FolderItem } from '../types';
import {
  X,
  Trash2,
  Plus,
  FolderOpen,
  Settings as SettingsIcon,
  Folder as FolderIcon,
  MoreHorizontal,
  BarChart3,
  Database,
  ImageIcon,
  CalendarDays,
  HardDrive,
} from 'lucide-react';
import { useState, useEffect } from 'react';
import { useTheme } from '../hooks/useTheme';
import { invoke } from '@tauri-apps/api/core';
import { emit } from '@tauri-apps/api/event';
import { getVersion } from '@tauri-apps/api/app';
import { openUrl } from '@tauri-apps/plugin-opener';
import { check } from '@tauri-apps/plugin-updater';
import { relaunch } from '@tauri-apps/plugin-process';
import { toast } from 'sonner';
import { ConfirmDialog } from './ConfirmDialog';
import { useShortcutRecorder } from 'use-shortcut-recorder';
import { clsx } from 'clsx';

interface SettingsPanelProps {
  settings: Settings;
  onClose: () => void;
}

type Tab = 'dashboard' | 'general' | 'folders';

interface DashboardStats {
  total: number;
  today: number;
  images: number;
  folders: number;
  daily: { day: string; count: number }[];
  top_apps: { app: string; count: number }[];
  most_pasted: { id: string; preview: string; count: number }[];
  db_size: number;
  images_size: number;
}



function formatBytes(bytes: number): string {
  if (bytes < 1024) return `${bytes} B`;
  if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(1)} KB`;
  return `${(bytes / (1024 * 1024)).toFixed(1)} MB`;
}

function formatTime(isoStr: string): string {
  try {
    const d = new Date(isoStr);
    return d.toLocaleTimeString([], { hour: '2-digit', minute: '2-digit' });
  } catch { return ''; }
}

function toDateStr(d: Date): string {
  return d.toISOString().split('T')[0];
}

function getDayLabel(dateStr: string): string {
  const d = new Date(dateStr + 'T00:00:00');
  const days = ['CN', 'T2', 'T3', 'T4', 'T5', 'T6', 'T7'];
  return days[d.getDay()];
}

export function SettingsPanel({ settings: initialSettings, onClose }: SettingsPanelProps) {
  const [activeTab, setActiveTab] = useState<Tab>('dashboard');
  const [settings, setSettings] = useState<Settings>(initialSettings);
  const [_historySize, setHistorySize] = useState<number>(0);
  const [isRecordingMode, setIsRecordingMode] = useState(false);

  // Folder Management State
  const [folders, setFolders] = useState<FolderItem[]>([]);
  const [newFolderName, setNewFolderName] = useState('');
  const [editingFolderId, setEditingFolderId] = useState<string | null>(null);
  const [renameValue, setRenameValue] = useState('');

  // Data Directory State
  const [dataDirectory, setDataDirectory] = useState<string>('');

  // Dashboard State
  const [dashStats, setDashStats] = useState<DashboardStats | null>(null);
  const [dashDate, setDashDate] = useState(toDateStr(new Date()));
  const [dashSearch, setDashSearch] = useState('');
  const [dashClips, setDashClips] = useState<{ id: string; clip_type: string; content: string; preview: string; created_at: string; source_app: string | null; subtype: string | null }[]>([]);
  const [dashClipsLoading, setDashClipsLoading] = useState(false);

  // Apply theme immediately when settings.theme changes
  useTheme(settings.theme);

  // Generic handler for immediate settings updates
  const updateSettings = async (updates: Partial<Settings>) => {
    // Determine the next state before updating React state
    setSettings((prev) => {
      const newSettings = { ...prev, ...updates };

      // Schedule async actions - we use newSettings which is local to this scope
      // This avoids race conditions with 'settings' variable
      (async () => {
        try {
          await invoke('save_settings', { settings: newSettings });
          await emit('settings-changed', newSettings);

          if (updates.hotkey) {
            await invoke('register_global_shortcut', { hotkey: updates.hotkey });
          }
        } catch (error) {
          console.error(`Failed to save settings:`, error);
          toast.error(`Failed to save settings`);
        }
      })();

      // Feedback for changes
      const keys = Object.keys(updates);
      if (keys.length === 1) {
        const key = keys[0] as keyof Settings;
        const value = updates[key];
        if (key !== 'theme') {
          const label = key
            .split('_')
            .map((w) => w.charAt(0).toUpperCase() + w.slice(1))
            .join(' ');
          if (typeof value === 'boolean') {
            toast.success(`${label} was ${value ? 'enabled' : 'disabled'}`);
          } else {
            toast.success(`${label} updated`);
          }
        }
      } else if (keys.length > 1) {
        toast.success('Settings updated');
      }

      return newSettings;
    });
  };

  const updateSetting = (key: keyof Settings, value: any) => {
    updateSettings({ [key]: value });
  };

  const handleThemeChange = (newTheme: string) => {
    updateSetting('theme', newTheme);
  };

  // Use use-shortcut-recorder for recording (shows current keys held in real-time)
  const {
    shortcut,
    savedShortcut,
    startRecording: startRecordingLib,
    stopRecording: stopRecordingLib,
    clearLastRecording,
  } = useShortcutRecorder({
    minModKeys: 1, // Require at least one modifier
  });

  // Start recording mode
  const handleStartRecording = () => {
    setIsRecordingMode(true);
    startRecordingLib();
  };

  const [ignoredApps, setIgnoredApps] = useState<string[]>([]);
  const [newIgnoredApp, setNewIgnoredApp] = useState('');
  const [appVersion, setAppVersion] = useState('');

  // Confirmation Dialog State
  const [confirmDialog, setConfirmDialog] = useState({
    isOpen: false,
    title: '',
    message: '',
    action: async () => {},
  });

  const loadFolders = async () => {
    try {
      const data = await invoke<FolderItem[]>('get_folders');
      setFolders(data);
    } catch (error) {
      console.error('Failed to load folders:', error);
    }
  };

  useEffect(() => {
    invoke<number>('get_clipboard_history_size').then(setHistorySize).catch(console.error);
    invoke<string[]>('get_ignored_apps').then(setIgnoredApps).catch(console.error);
    getVersion().then(setAppVersion).catch(console.error);
    loadFolders();
    invoke<string>('get_data_directory').then(setDataDirectory).catch(console.error);
    invoke<DashboardStats>('get_dashboard_stats').then(setDashStats).catch(console.error);
  }, []);

  // Load clips for selected date
  useEffect(() => {
    if (activeTab !== 'dashboard') return;
    setDashClipsLoading(true);
    const search = dashSearch.trim() || undefined;
    invoke<typeof dashClips>('get_clips_by_date', { date: dashDate, search })
      .then(setDashClips)
      .catch(console.error)
      .finally(() => setDashClipsLoading(false));
  }, [dashDate, dashSearch, activeTab]);

  const handleAddIgnoredApp = async () => {
    if (!newIgnoredApp.trim()) return;
    try {
      await invoke('add_ignored_app', { appName: newIgnoredApp.trim() });
      setIgnoredApps((prev) => [...prev, newIgnoredApp.trim()].sort());
      setNewIgnoredApp('');
      toast.success(`Added ${newIgnoredApp.trim()} to ignored apps`);
    } catch (e) {
      toast.error(`Failed to add ignored app: ${e}`);
      console.error(e);
    }
  };

  const handleBrowseFile = async () => {
    try {
      const path = await invoke<string>('pick_file');
      const filename = path.split('\\').pop() || path;
      setNewIgnoredApp(filename);
    } catch (e) {
      console.log('File picker cancelled or failed', e);
    }
  };

  const handleRemoveIgnoredApp = async (app: string) => {
    try {
      await invoke('remove_ignored_app', { appName: app });
      setIgnoredApps((prev) => prev.filter((a) => a !== app));
      toast.success(`Removed ${app} from ignored apps`);
    } catch (e) {
      toast.error(`Failed to remove ignored app: ${e}`);
      console.error(e);
    }
  };

  const confirmClearHistory = () => {
    setConfirmDialog({
      isOpen: true,
      title: 'Clear History',
      message:
        'Are you sure you want to clear your clipboard history? This will only remove items that are not in folders. Items saved in folders will be preserved.',
      action: async () => {
        try {
          await invoke('clear_all_clips');
          // Refresh the history size after clearing
          const newSize = await invoke<number>('get_clipboard_history_size');
          setHistorySize(newSize);
          toast.success('Clipboard history cleared successfully.');
        } catch (error) {
          console.error('Failed to clear history:', error);
          toast.error(`Failed to clear history: ${error}`);
        }
      },
    });
  };

  // Folder Management Functions
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

  // Format shortcut array into Tauri-compatible string
  const formatHotkey = (keys: string[]): string => {
    return keys
      .map((k) => {
        if (k === 'Control') return 'Ctrl';
        if (k === 'Alt') return 'Alt';
        if (k === 'Shift') return 'Shift';
        if (k === 'Meta') return 'Cmd';
        if (k.startsWith('Key')) return k.slice(3);
        if (k.startsWith('Digit')) return k.slice(5);
        return k;
      })
      .join('+');
  };

  const handleSaveHotkey = async () => {
    if (savedShortcut.length > 0) {
      const newHotkey = formatHotkey(savedShortcut);
      await updateSetting('hotkey', newHotkey);
    }
    stopRecordingLib();
    setIsRecordingMode(false);
  };

  const handleCancelRecording = () => {
    stopRecordingLib();
    clearLastRecording();
    setIsRecordingMode(false);
  };

  const [updateProgress, setUpdateProgress] = useState<{ percent: number; downloaded: number; total: number } | null>(null);

  const handleCheckUpdate = async () => {
    try {
      const loadingToast = toast.loading('Checking for updates...');
      const update = await check();
      toast.dismiss(loadingToast);

      if (update && update.available) {
        toast.info(`Update v${update.version} available!`, {
          duration: 10000,
          action: {
            label: 'Download & Restart',
            onClick: async () => {
              try {
                setUpdateProgress({ percent: 0, downloaded: 0, total: 0 });
                let totalBytes = 0;
                let downloadedBytes = 0;

                await update.downloadAndInstall((event) => {
                  if (event.event === 'Started' && event.data.contentLength) {
                    totalBytes = event.data.contentLength;
                    setUpdateProgress({ percent: 0, downloaded: 0, total: totalBytes });
                  } else if (event.event === 'Progress') {
                    downloadedBytes += event.data.chunkLength;
                    const percent = totalBytes > 0 ? Math.round((downloadedBytes / totalBytes) * 100) : 0;
                    setUpdateProgress({ percent, downloaded: downloadedBytes, total: totalBytes });
                  } else if (event.event === 'Finished') {
                    setUpdateProgress({ percent: 100, downloaded: totalBytes, total: totalBytes });
                  }
                });

                setUpdateProgress(null);
                toast.success('Update installed. Restarting...');
                await relaunch();
              } catch (e) {
                setUpdateProgress(null);
                toast.error(`Update failed: ${e}`);
              }
            },
          },
        });
      } else {
        toast.success('You are on the latest version.');
      }
    } catch (e) {
      toast.error(`Check failed: ${e}`);
    }
  };

  const handleSelectDataDirectory = async () => {
    try {
      const selectedPath = await invoke<string>('pick_folder');
      if (selectedPath) {
        await invoke('set_data_directory', { newPath: selectedPath });
        setDataDirectory(selectedPath);
        toast.success('Data directory changed. Please restart the application for changes to take effect.', {
          duration: 5000,
        });
      }
    } catch (e) {
      console.error('Failed to select data directory:', e);
      toast.error(`Failed to select folder: ${e}`);
    }
  };

  return (
    <>
      <ConfirmDialog
        isOpen={confirmDialog.isOpen}
        title={confirmDialog.title}
        message={confirmDialog.message}
        onConfirm={async () => {
          await confirmDialog.action();
          setConfirmDialog((prev) => ({ ...prev, isOpen: false }));
        }}
        onCancel={() => setConfirmDialog((prev) => ({ ...prev, isOpen: false }))}
      />
      <div className="flex h-full flex-col bg-background text-foreground">
        {/* Header */}
        <div className="drag-area flex items-center justify-between border-b border-border p-4">
          <h2 className="text-lg font-semibold">Settings</h2>
          <button onClick={onClose} className="no-drag icon-button" style={{ WebkitAppRegion: 'no-drag' } as any}>
            <X size={18} />
          </button>
        </div>

        <div className="flex flex-1 overflow-hidden">
          {/* Sidebar */}
          <div className="w-48 flex-shrink-0 border-r border-border bg-card/50 p-2">
            <div className="flex flex-col gap-1">
              <button
                onClick={() => setActiveTab('dashboard')}
                className={clsx(
                  'flex items-center gap-2 rounded-md px-3 py-2 text-sm font-medium transition-colors',
                  activeTab === 'dashboard'
                    ? 'bg-accent text-accent-foreground'
                    : 'text-muted-foreground hover:bg-accent/50 hover:text-foreground'
                )}
              >
                <BarChart3 size={16} />
                Dashboard
              </button>
              <button
                onClick={() => setActiveTab('general')}
                className={clsx(
                  'flex items-center gap-2 rounded-md px-3 py-2 text-sm font-medium transition-colors',
                  activeTab === 'general'
                    ? 'bg-accent text-accent-foreground'
                    : 'text-muted-foreground hover:bg-accent/50 hover:text-foreground'
                )}
              >
                <SettingsIcon size={16} />
                General
              </button>
              <button
                onClick={() => setActiveTab('folders')}
                className={clsx(
                  'flex items-center gap-2 rounded-md px-3 py-2 text-sm font-medium transition-colors',
                  activeTab === 'folders'
                    ? 'bg-accent text-accent-foreground'
                    : 'text-muted-foreground hover:bg-accent/50 hover:text-foreground'
                )}
              >
                <FolderIcon size={16} />
                Folders
              </button>
            </div>
          </div>

          {/* Content Area */}
          <div className="flex-1 overflow-y-auto p-6">
            <div className="mx-auto max-w-2xl space-y-8">
              {/* --- DASHBOARD TAB --- */}
              {activeTab === 'dashboard' && (
                <>
                  {/* Stats Cards */}
                  {dashStats && (
                    <section className="grid grid-cols-4 gap-3">
                      <div className="flex flex-col items-center rounded-xl border border-border bg-card/50 p-3">
                        <Database size={16} className="mb-1 text-indigo-400" />
                        <span className="text-xl font-bold text-indigo-400">{dashStats.total.toLocaleString()}</span>
                        <span className="text-[10px] text-muted-foreground">Total</span>
                      </div>
                      <div className="flex flex-col items-center rounded-xl border border-border bg-card/50 p-3">
                        <CalendarDays size={16} className="mb-1 text-emerald-400" />
                        <span className="text-xl font-bold text-emerald-400">{dashStats.today}</span>
                        <span className="text-[10px] text-muted-foreground">Today</span>
                      </div>
                      <div className="flex flex-col items-center rounded-xl border border-border bg-card/50 p-3">
                        <ImageIcon size={16} className="mb-1 text-cyan-400" />
                        <span className="text-xl font-bold text-cyan-400">{dashStats.images}</span>
                        <span className="text-[10px] text-muted-foreground">Images</span>
                      </div>
                      <div className="flex flex-col items-center rounded-xl border border-border bg-card/50 p-3">
                        <FolderIcon size={16} className="mb-1 text-amber-400" />
                        <span className="text-xl font-bold text-amber-400">{dashStats.folders}</span>
                        <span className="text-[10px] text-muted-foreground">Folders</span>
                      </div>
                    </section>
                  )}

                  {/* Date picker + Search */}
                  <section className="space-y-3">
                    <h3 className="text-sm font-medium text-muted-foreground">History Timeline</h3>
                    <div className="flex gap-2">
                      <div className="flex items-center gap-1">
                        <button
                          onClick={() => { const d = new Date(dashDate + 'T00:00:00'); d.setDate(d.getDate() - 1); setDashDate(toDateStr(d)); }}
                          className="rounded-md px-2 py-1.5 text-sm hover:bg-accent"
                        >
                          ‹
                        </button>
                        <input
                          type="date"
                          value={dashDate}
                          onChange={(e) => setDashDate(e.target.value)}
                          max={toDateStr(new Date())}
                          className="rounded-lg border border-border bg-input px-3 py-1.5 text-sm text-foreground focus:outline-none focus:ring-1 focus:ring-ring"
                          style={{ colorScheme: 'dark' }}
                        />
                        <button
                          onClick={() => { const d = new Date(dashDate + 'T00:00:00'); d.setDate(d.getDate() + 1); if (d <= new Date()) setDashDate(toDateStr(d)); }}
                          className="rounded-md px-2 py-1.5 text-sm hover:bg-accent"
                        >
                          ›
                        </button>
                        <button
                          onClick={() => setDashDate(toDateStr(new Date()))}
                          className="rounded-md px-2 py-1 text-xs text-muted-foreground hover:bg-accent"
                        >
                          Today
                        </button>
                      </div>
                      <input
                        type="text"
                        value={dashSearch}
                        onChange={(e) => setDashSearch(e.target.value)}
                        placeholder="Search in this day..."
                        className="flex-1 rounded-lg border border-border bg-input px-3 py-1.5 text-sm focus:outline-none focus:ring-1 focus:ring-ring"
                      />
                    </div>
                  </section>

                  {/* Clips for selected date */}
                  <section className="space-y-2">
                    <div className="flex items-center justify-between">
                      <span className="text-xs text-muted-foreground">
                        {dashDate === toDateStr(new Date()) ? 'Today' : dashDate} — {dashClips.length} clips
                      </span>
                    </div>
                    {dashClipsLoading ? (
                      <div className="flex items-center justify-center py-8">
                        <div className="h-6 w-6 animate-spin rounded-full border-2 border-primary/30 border-t-primary" />
                      </div>
                    ) : dashClips.length === 0 ? (
                      <div className="rounded-lg border border-border/50 bg-card/30 py-6 text-center text-sm text-muted-foreground/50">
                        No clips on this day
                      </div>
                    ) : (
                      <div className="max-h-[300px] space-y-1 overflow-y-auto rounded-lg border border-border/50 bg-card/30 p-2">
                        {dashClips.map((clip) => (
                          <div key={clip.id} className="flex items-center gap-3 rounded-md px-2 py-1.5 hover:bg-accent/30">
                            {/* Time */}
                            <span className="w-12 flex-shrink-0 text-[11px] tabular-nums text-muted-foreground/60">
                              {formatTime(clip.created_at)}
                            </span>
                            {/* Type badge */}
                            {clip.clip_type === 'image' ? (
                              <div className="flex h-8 w-10 flex-shrink-0 items-center justify-center overflow-hidden rounded border border-border/30">
                                <img src={`data:image/png;base64,${clip.content.substring(0, 200)}`} alt="" className="h-full w-full object-cover" />
                              </div>
                            ) : (
                              <div className={clsx(
                                'flex h-5 w-5 flex-shrink-0 items-center justify-center rounded text-[9px] font-bold',
                                clip.subtype === 'url' ? 'bg-blue-500/20 text-blue-400' :
                                clip.subtype === 'email' ? 'bg-emerald-500/20 text-emerald-400' :
                                clip.subtype === 'color' ? 'bg-pink-500/20 text-pink-400' :
                                clip.subtype === 'path' ? 'bg-amber-500/20 text-amber-400' :
                                'bg-muted/30 text-muted-foreground/60'
                              )}>
                                {clip.subtype === 'url' ? '🔗' : clip.subtype === 'email' ? '✉' : clip.subtype === 'color' ? '🎨' : clip.subtype === 'path' ? '📁' : 'T'}
                              </div>
                            )}
                            {/* Content preview */}
                            <span className="flex-1 truncate font-mono text-xs text-foreground/80">
                              {clip.clip_type === 'image' ? '[Image]' : clip.preview?.substring(0, 100) || clip.content.substring(0, 100)}
                            </span>
                            {/* Source app */}
                            {clip.source_app && (
                              <span className="flex-shrink-0 truncate text-[10px] text-muted-foreground/40" style={{ maxWidth: 80 }}>
                                {clip.source_app}
                              </span>
                            )}
                          </div>
                        ))}
                      </div>
                    )}
                  </section>

                  {/* Activity Chart */}
                  {dashStats && dashStats.daily.length > 0 && (
                    <section className="space-y-3">
                      <h3 className="text-sm font-medium text-muted-foreground">Activity (last 7 days)</h3>
                      <div className="rounded-xl border border-border bg-card/50 p-4">
                        {(() => {
                          const maxCount = Math.max(...dashStats.daily.map(d => d.count), 1);
                          return (
                            <div className="flex items-end gap-2" style={{ height: 80 }}>
                              {dashStats.daily.map((d) => (
                                <div key={d.day} className="flex flex-1 flex-col items-center gap-1 cursor-pointer" onClick={() => setDashDate(d.day)}>
                                  <span className="text-[9px] text-muted-foreground/70">{d.count}</span>
                                  <div
                                    className={clsx(
                                      'w-full rounded-t-md transition-all',
                                      d.day === dashDate ? 'bg-indigo-400' : 'bg-indigo-500/40 hover:bg-indigo-500/60'
                                    )}
                                    style={{ height: `${(d.count / maxCount) * 60}px`, minHeight: d.count > 0 ? 4 : 0 }}
                                  />
                                  <span className={clsx('text-[9px]', d.day === dashDate ? 'text-indigo-400 font-bold' : 'text-muted-foreground')}>{getDayLabel(d.day)}</span>
                                </div>
                              ))}
                            </div>
                          );
                        })()}
                      </div>
                    </section>
                  )}

                  {/* Top Source Apps */}
                  {dashStats && dashStats.top_apps.length > 0 && (
                    <section className="space-y-3">
                      <h3 className="text-sm font-medium text-muted-foreground">Top source apps</h3>
                      <div className="space-y-2">
                        {(() => {
                          const maxApp = Math.max(...dashStats.top_apps.map(a => a.count), 1);
                          return dashStats.top_apps.map((app) => (
                            <div key={app.app} className="flex items-center gap-3">
                              <div className="flex h-6 w-6 items-center justify-center rounded-md bg-indigo-500/20 text-[9px] font-bold text-indigo-300">
                                {app.app.substring(0, 2).toUpperCase()}
                              </div>
                              <span className="w-20 truncate text-xs font-medium">{app.app}</span>
                              <div className="flex-1">
                                <div className="h-2 rounded-full bg-gradient-to-r from-indigo-500/80 to-purple-500/60" style={{ width: `${(app.count / maxApp) * 100}%` }} />
                              </div>
                              <span className="w-8 text-right text-[10px] text-muted-foreground">{app.count}</span>
                            </div>
                          ));
                        })()}
                      </div>
                    </section>
                  )}

                  {/* Most Pasted + Storage */}
                  {dashStats && (
                    <div className="grid grid-cols-2 gap-4">
                      {dashStats.most_pasted.length > 0 && (
                        <section className="space-y-2">
                          <h3 className="text-xs font-medium text-muted-foreground">Most pasted</h3>
                          <div className="space-y-1">
                            {dashStats.most_pasted.map((clip, i) => (
                              <div key={clip.id} className="flex items-center gap-2 rounded px-1.5 py-1 hover:bg-accent/30">
                                <span className="text-[10px] text-muted-foreground/50">{i + 1}.</span>
                                <span className="flex-1 truncate font-mono text-[10px] text-foreground/70">{clip.preview}</span>
                                <span className="text-[10px] font-semibold text-emerald-400">{clip.count}x</span>
                              </div>
                            ))}
                          </div>
                        </section>
                      )}
                      <section className="space-y-2">
                        <h3 className="text-xs font-medium text-muted-foreground">Storage</h3>
                        <div className="space-y-2">
                          <div className="flex items-center gap-2 rounded-lg border border-border bg-card/50 px-3 py-2">
                            <HardDrive size={12} className="text-muted-foreground" />
                            <span className="text-xs font-medium">{formatBytes(dashStats.db_size)}</span>
                            <span className="text-[10px] text-muted-foreground">DB</span>
                          </div>
                          <div className="flex items-center gap-2 rounded-lg border border-border bg-card/50 px-3 py-2">
                            <ImageIcon size={12} className="text-muted-foreground" />
                            <span className="text-xs font-medium">{formatBytes(dashStats.images_size)}</span>
                            <span className="text-[10px] text-muted-foreground">Images</span>
                          </div>
                        </div>
                      </section>
                    </div>
                  )}
                </>
              )}

              {/* --- GENERAL TAB --- */}
              {activeTab === 'general' && (
                <>
                  <section className="space-y-4">
                    <h3 className="text-sm font-medium text-muted-foreground">
                      Appearance & Behavior
                    </h3>

                    <div className="grid grid-cols-2 gap-4">
                      <div className="space-y-3">
                        <label className="block">
                          <span className="text-sm font-medium">Theme</span>
                        </label>
                        <select
                          value={settings.theme}
                          onChange={(e) => handleThemeChange(e.target.value)}
                          className="w-full rounded-lg border border-border bg-input px-3 py-2 text-sm text-foreground focus:outline-none focus:ring-2 focus:ring-ring"
                        >
                          <option value="dark">Dark</option>
                          <option value="light">Light</option>
                          <option value="system">System</option>
                        </select>
                      </div>

                      <div className="space-y-3">
                        <label className="block">
                          <span className="text-sm font-medium">Window Effect</span>
                        </label>
                        <select
                          value={settings.mica_effect || 'clear'}
                          onChange={(e) => updateSetting('mica_effect', e.target.value)}
                          className="w-full rounded-lg border border-border bg-input px-3 py-2 text-sm text-foreground focus:outline-none focus:ring-2 focus:ring-ring"
                        >
                          <option value="mica_alt">Mica Alt</option>
                          <option value="mica">Mica</option>
                          <option value="clear">Clear</option>
                        </select>
                      </div>
                    </div>

                    <div className="flex items-center justify-between rounded-lg border border-border bg-accent/20 p-3">
                      <div>
                        <span className="text-sm font-medium">Startup with Windows</span>
                        <p className="text-xs text-muted-foreground">
                          Automatically start when Windows boots
                        </p>
                      </div>
                      <button
                        onClick={() =>
                          updateSetting('startup_with_windows', !settings.startup_with_windows)
                        }
                        className={`h-6 w-11 rounded-full transition-colors ${settings.startup_with_windows ? 'bg-primary' : 'bg-accent'}`}
                      >
                        <div
                          className={`h-5 w-5 rounded-full bg-white shadow-sm transition-transform ${settings.startup_with_windows ? 'translate-x-5' : 'translate-x-0.5'}`}
                        />
                      </button>
                    </div>

                    <div className="flex items-center justify-between rounded-lg border border-border bg-accent/20 p-3">
                      <div>
                        <span className="text-sm font-medium">Auto Paste</span>
                        <p className="text-xs text-muted-foreground">
                          Automatically paste when selecting a clip
                        </p>
                      </div>
                      <button
                        onClick={() => updateSetting('auto_paste', !settings.auto_paste)}
                        className={`h-6 w-11 rounded-full transition-colors ${settings.auto_paste ? 'bg-primary' : 'bg-accent'}`}
                      >
                        <div
                          className={`h-5 w-5 rounded-full bg-white shadow-sm transition-transform ${settings.auto_paste ? 'translate-x-5' : 'translate-x-0.5'}`}
                        />
                      </button>
                    </div>

                    <div className="flex items-center justify-between rounded-lg border border-border bg-accent/20 p-3">
                      <div>
                        <span className="text-sm font-medium">Ignore Ghost Clips</span>
                        <p className="text-xs text-muted-foreground">
                          Ignore content from unknown background apps
                        </p>
                      </div>
                      <button
                        onClick={() =>
                          updateSetting('ignore_ghost_clips', !settings.ignore_ghost_clips)
                        }
                        className={`h-6 w-11 rounded-full transition-colors ${settings.ignore_ghost_clips ? 'bg-primary' : 'bg-accent'}`}
                      >
                        <div
                          className={`h-5 w-5 rounded-full bg-white shadow-sm transition-transform ${settings.ignore_ghost_clips ? 'translate-x-5' : 'translate-x-0.5'}`}
                        />
                      </button>
                    </div>
                  </section>

                  <section className="space-y-4">
                    <h3 className="text-sm font-medium text-muted-foreground">Shortcuts</h3>
                    <div className="space-y-3">
                      <label className="block">
                        <span className="text-sm font-medium">Global Hotkey</span>
                        <p className="text-xs text-muted-foreground">Toggle the clipboard window</p>
                      </label>
                      {isRecordingMode ? (
                        <div className="space-y-2">
                          <div className="flex w-full items-center gap-2 rounded-lg border border-primary bg-input px-3 py-2 text-sm ring-2 ring-primary">
                            <span className="animate-pulse text-primary">
                              {shortcut.length > 0
                                ? formatHotkey(shortcut)
                                : savedShortcut.length > 0
                                  ? formatHotkey(savedShortcut)
                                  : 'Press keys...'}
                            </span>
                          </div>
                          <div className="flex gap-2">
                            <button
                              onClick={handleSaveHotkey}
                              disabled={savedShortcut.length === 0}
                              className="rounded bg-primary px-3 py-1 text-xs text-primary-foreground disabled:opacity-50"
                            >
                              Save
                            </button>
                            <button
                              onClick={handleCancelRecording}
                              className="rounded bg-muted px-3 py-1 text-xs text-muted-foreground"
                            >
                              Cancel
                            </button>
                          </div>
                        </div>
                      ) : (
                        <button
                          onClick={handleStartRecording}
                          className="flex w-full items-center gap-2 rounded-lg border border-border bg-input px-3 py-2 text-sm transition-colors hover:border-primary"
                        >
                          <span className="rounded bg-accent px-2 py-0.5 font-mono text-xs font-medium">
                            {settings.hotkey}
                          </span>
                        </button>
                      )}
                    </div>
                  </section>

                  <section className="space-y-4">
                    <h3 className="text-sm font-medium text-muted-foreground">
                      Privacy Exceptions
                    </h3>
                    <div className="space-y-3">
                      <label className="block">
                        <span className="text-sm font-medium">Ignored Applications</span>
                        <p className="text-xs text-muted-foreground">
                          Prevent recording from specific apps (filename or path).
                        </p>
                      </label>

                      <div className="flex gap-2">
                        <input
                          type="text"
                          value={newIgnoredApp}
                          onChange={(e) => setNewIgnoredApp(e.target.value)}
                          placeholder="e.g. notepad.exe"
                          className="flex-1 rounded-lg border border-border bg-input px-3 py-2 text-sm focus:outline-none focus:ring-2 focus:ring-ring"
                          onKeyDown={(e) => e.key === 'Enter' && handleAddIgnoredApp()}
                        />
                        <button
                          onClick={handleBrowseFile}
                          className="btn btn-secondary px-3"
                          title="Browse executable"
                        >
                          <FolderOpen size={16} />
                        </button>
                        <button
                          onClick={handleAddIgnoredApp}
                          disabled={!newIgnoredApp.trim()}
                          className="btn btn-secondary px-3"
                          title="Add to list"
                        >
                          <Plus size={16} />
                        </button>
                      </div>

                      <div className="max-h-40 space-y-1 overflow-y-auto pr-1">
                        {ignoredApps.length === 0 ? (
                          <div className="rounded-lg border border-dashed border-border p-4 text-center">
                            <p className="text-xs text-muted-foreground">No ignored applications</p>
                          </div>
                        ) : (
                          ignoredApps.map((app) => (
                            <div
                              key={app}
                              className="group flex items-center justify-between rounded-md border border-transparent bg-accent/30 px-3 py-2 text-sm hover:border-border hover:bg-accent/50"
                            >
                              <span className="font-mono text-xs">{app}</span>
                              <button
                                onClick={() => handleRemoveIgnoredApp(app)}
                                className="text-muted-foreground opacity-0 transition-opacity hover:text-destructive group-hover:opacity-100"
                              >
                                <X size={14} />
                              </button>
                            </div>
                          ))
                        )}
                      </div>
                    </div>
                  </section>

                  <section className="space-y-4">
                    <h3 className="text-sm font-medium text-muted-foreground">Data Storage</h3>
                    <div className="space-y-3">
                      <label className="block">
                        <span className="text-sm font-medium">Data Directory</span>
                        <p className="text-xs text-muted-foreground">
                          Choose where to store clipboard database (e.g., Google Drive folder for sync)
                        </p>
                      </label>
                      <div className="flex gap-2">
                        <input
                          type="text"
                          value={dataDirectory}
                          readOnly
                          className="flex-1 rounded-lg border border-border bg-input px-3 py-2 text-sm text-muted-foreground focus:outline-none"
                          placeholder="Default location"
                        />
                        <button
                          onClick={handleSelectDataDirectory}
                          className="btn btn-secondary px-4"
                          title="Choose folder"
                        >
                          <FolderOpen size={16} className="mr-2" />
                          Choose Folder
                        </button>
                      </div>
                      <p className="text-xs text-muted-foreground">
                        Current: {dataDirectory || 'Default location'}
                      </p>
                    </div>
                  </section>

                  <section className="space-y-4">
                    <h3 className="text-sm font-medium text-red-500/80">Data Management</h3>
                    <div className="grid grid-cols-2 gap-3">
                      <button
                        onClick={confirmClearHistory}
                        className="btn border border-destructive/20 bg-destructive/10 text-destructive hover:bg-destructive/20"
                      >
                        <Trash2 size={16} className="mr-2" />
                        Clear History
                      </button>

                      <button
                        onClick={async () => {
                          try {
                            const count = await invoke<number>('remove_duplicate_clips');
                            toast.success(`Removed ${count} duplicate clips`);
                            const newSize = await invoke<number>('get_clipboard_history_size');
                            setHistorySize(newSize);
                          } catch (error) {
                            console.error(error);
                            toast.error(`Failed to remove duplicates: ${error}`);
                          }
                        }}
                        className="btn btn-secondary text-xs"
                      >
                        Remove Duplicates
                      </button>

                      <button
                        onClick={async () => {
                          try {
                            const path = await invoke<string>('export_data');
                            toast.success(`Exported to ${path}`);
                          } catch (error) {
                            if (String(error) !== 'Export cancelled') {
                              toast.error(`Export failed: ${error}`);
                            }
                          }
                        }}
                        className="btn btn-secondary text-xs"
                      >
                        Export Backup
                      </button>

                      <button
                        onClick={async () => {
                          try {
                            await invoke('import_data');
                            toast.success('Backup imported. Restart to apply.');
                          } catch (error) {
                            if (String(error) !== 'Import cancelled') {
                              toast.error(`Import failed: ${error}`);
                            }
                          }
                        }}
                        className="btn btn-secondary text-xs"
                      >
                        Import Backup
                      </button>
                    </div>
                  </section>
                </>
              )}


              {/* --- FOLDERS TAB --- */}
              {activeTab === 'folders' && (
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
              )}
            </div>
          </div>
        </div>

        {/* Footer */}
        <div className="flex flex-col items-center gap-1 border-t border-border bg-background px-4 py-3 text-center">
          <button
            onClick={() => openUrl('https://github.com/Phieu-Tran/ClipPaste').catch(console.error)}
            className="text-xs text-muted-foreground transition-colors hover:text-foreground"
          >
            ClipPaste {appVersion || '...'}
          </button>
          <div className="flex gap-2 text-xs text-muted-foreground">
            <span>© 2026</span>
            <span>•</span>
            <button onClick={handleCheckUpdate} className="underline hover:text-foreground" disabled={!!updateProgress}>
              {updateProgress ? 'Updating...' : 'Check for Updates'}
            </button>
          </div>
          {updateProgress && (
            <div className="mt-2 w-full max-w-[280px]">
              <div className="mb-1 flex justify-between text-[10px] text-muted-foreground">
                <span>{updateProgress.percent}%</span>
                <span>
                  {(updateProgress.downloaded / 1024 / 1024).toFixed(1)} / {(updateProgress.total / 1024 / 1024).toFixed(1)} MB
                </span>
              </div>
              <div className="h-1.5 w-full overflow-hidden rounded-full bg-muted">
                <div
                  className="h-full rounded-full bg-blue-500 transition-all duration-300"
                  style={{ width: `${updateProgress.percent}%` }}
                />
              </div>
            </div>
          )}
        </div>
      </div>
    </>
  );
}
