import { Settings, FolderItem } from '../types';
import {
  X,
  Settings as SettingsIcon,
  Folder as FolderIcon,
  BarChart3,
} from 'lucide-react';
import { useState, useEffect, useRef } from 'react';
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

import { DashboardTab } from './settings/DashboardTab';
import { GeneralTab } from './settings/GeneralTab';
import { FoldersTab } from './settings/FoldersTab';

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

function toDateStr(d: Date): string {
  return d.toISOString().split('T')[0];
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

  // Debounced dashboard search
  const dashSearchTimerRef = useRef<ReturnType<typeof setTimeout> | null>(null);
  const [debouncedDashSearch, setDebouncedDashSearch] = useState('');

  // Apply theme immediately when settings.theme changes
  useTheme(settings.theme);

  // Generic handler for immediate settings updates
  const updateSettings = async (updates: Partial<Settings>) => {
    const newSettings = { ...settings, ...updates };
    setSettings(newSettings);

    try {
      await invoke('save_settings', { settings: newSettings });
      await emit('settings-changed', newSettings);

      if (updates.hotkey) {
        await invoke('register_global_shortcut', { hotkey: updates.hotkey });
      }
    } catch (error) {
      console.error(`Failed to save settings:`, error);
      toast.error(`Failed to save settings`);
      return; // Don't show success toast
    }

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

  // Debounce dashboard search input
  useEffect(() => {
    if (dashSearchTimerRef.current) clearTimeout(dashSearchTimerRef.current);
    dashSearchTimerRef.current = setTimeout(() => {
      setDebouncedDashSearch(dashSearch);
    }, 200);
    return () => {
      if (dashSearchTimerRef.current) clearTimeout(dashSearchTimerRef.current);
    };
  }, [dashSearch]);

  // Load clips for selected date
  useEffect(() => {
    if (activeTab !== 'dashboard') return;
    setDashClipsLoading(true);
    const search = debouncedDashSearch.trim() || undefined;
    invoke<typeof dashClips>('get_clips_by_date', { date: dashDate, search })
      .then(setDashClips)
      .catch(console.error)
      .finally(() => setDashClipsLoading(false));
  }, [dashDate, debouncedDashSearch, activeTab]);

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
              {activeTab === 'dashboard' && (
                <DashboardTab
                  dashStats={dashStats}
                  dashDate={dashDate}
                  setDashDate={setDashDate}
                  dashSearch={dashSearch}
                  setDashSearch={setDashSearch}
                  dashClips={dashClips}
                  dashClipsLoading={dashClipsLoading}
                />
              )}

              {activeTab === 'general' && (
                <GeneralTab
                  settings={settings}
                  updateSetting={updateSetting}
                  handleThemeChange={handleThemeChange}
                  isRecordingMode={isRecordingMode}
                  shortcut={shortcut}
                  savedShortcut={savedShortcut}
                  formatHotkey={formatHotkey}
                  handleStartRecording={handleStartRecording}
                  handleSaveHotkey={handleSaveHotkey}
                  handleCancelRecording={handleCancelRecording}
                  ignoredApps={ignoredApps}
                  setIgnoredApps={setIgnoredApps}
                  newIgnoredApp={newIgnoredApp}
                  setNewIgnoredApp={setNewIgnoredApp}
                  dataDirectory={dataDirectory}
                  handleSelectDataDirectory={handleSelectDataDirectory}
                  setHistorySize={setHistorySize}
                  confirmClearHistory={confirmClearHistory}
                  updateProgress={updateProgress}
                  handleCheckUpdate={handleCheckUpdate}
                  appVersion={appVersion}
                />
              )}

              {activeTab === 'folders' && (
                <FoldersTab
                  folders={folders}
                  newFolderName={newFolderName}
                  setNewFolderName={setNewFolderName}
                  editingFolderId={editingFolderId}
                  setEditingFolderId={setEditingFolderId}
                  renameValue={renameValue}
                  setRenameValue={setRenameValue}
                  loadFolders={loadFolders}
                />
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
            <span>&copy; 2026</span>
            <span>&bull;</span>
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
