import { Settings } from '../../types';
import { useState } from 'react';
import {
  X,
  Trash2,
  Plus,
  FolderOpen,
  Crosshair,
} from 'lucide-react';
import { invoke } from '@tauri-apps/api/core';
import { toast } from 'sonner';

interface PickedApp {
  app_name: string | null;
  exe_name: string | null;
  full_path: string | null;
}

interface GeneralTabProps {
  settings: Settings;
  updateSetting: (key: keyof Settings, value: any) => void;
  handleThemeChange: (newTheme: string) => void;
  // Hotkey
  isRecordingMode: boolean;
  shortcut: string[];
  savedShortcut: string[];
  formatHotkey: (keys: string[]) => string;
  handleStartRecording: () => void;
  handleSaveHotkey: () => void;
  handleCancelRecording: () => void;
  // Ignored apps
  ignoredApps: string[];
  setIgnoredApps: React.Dispatch<React.SetStateAction<string[]>>;
  newIgnoredApp: string;
  setNewIgnoredApp: (v: string) => void;
  // Data directory
  dataDirectory: string;
  handleSelectDataDirectory: () => void;
  // History
  setHistorySize: React.Dispatch<React.SetStateAction<number>>;
  confirmClearHistory: () => void;
  // Update
  updateProgress: { percent: number; downloaded: number; total: number } | null;
  handleCheckUpdate: () => void;
  // App version
  appVersion: string;
}

export function GeneralTab({
  settings,
  updateSetting,
  handleThemeChange,
  isRecordingMode,
  shortcut,
  savedShortcut,
  formatHotkey,
  handleStartRecording,
  handleSaveHotkey,
  handleCancelRecording,
  ignoredApps,
  setIgnoredApps,
  newIgnoredApp,
  setNewIgnoredApp,
  dataDirectory,
  handleSelectDataDirectory,
  setHistorySize,
  confirmClearHistory,
}: GeneralTabProps) {

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

  // Target mode: countdown that captures whichever app is focused when it expires.
  const [targetCountdown, setTargetCountdown] = useState<number | null>(null);

  const handleTargetApp = async () => {
    if (targetCountdown !== null) return;
    const DELAY_SEC = 4;
    setTargetCountdown(DELAY_SEC);
    toast.info(`Switch to the app you want to block — capturing in ${DELAY_SEC}s`);

    const tick = setInterval(() => {
      setTargetCountdown((v) => (v !== null && v > 1 ? v - 1 : v));
    }, 1000);

    try {
      const picked = await invoke<PickedApp>('pick_foreground_app', { delayMs: DELAY_SEC * 1000 });
      // Prefer exe name (what the ignore check compares against). Fall back to display name.
      const target = picked.exe_name || picked.app_name || '';
      if (!target || target.toLowerCase().includes('clippaste')) {
        toast.error('Could not capture a different app — try again and switch to the target app before the countdown ends.');
      } else {
        setNewIgnoredApp(target);
        toast.success(`Captured: ${target} — click + to block`);
      }
    } catch (e) {
      toast.error(`Failed to capture app: ${e}`);
    } finally {
      clearInterval(tick);
      setTargetCountdown(null);
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

  return (
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
              <option value="acrylic">Acrylic</option>
              <option value="blur">Blur</option>
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
              onClick={handleTargetApp}
              disabled={targetCountdown !== null}
              className="btn btn-secondary px-3"
              title="Target a running app — switch to it within the countdown"
            >
              {targetCountdown !== null ? (
                <span className="text-xs font-semibold">{targetCountdown}s</span>
              ) : (
                <Crosshair size={16} />
              )}
            </button>
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
        <h3 className="text-sm font-medium text-muted-foreground">Clip Limits</h3>
        <div className="space-y-3">
          <div className="flex items-center justify-between">
            <span className="text-sm font-medium">Max Clips</span>
            <select
              value={[0, 500, 1000, 2000, 5000, 10000].includes(settings.max_items) ? settings.max_items : 'custom'}
              onChange={(e) => {
                const v = e.target.value;
                if (v === 'custom') updateSetting('max_items', 1000);
                else updateSetting('max_items', parseInt(v));
              }}
              className="rounded-lg border border-border bg-input px-3 py-1.5 text-sm text-foreground focus:outline-none focus:ring-1 focus:ring-ring"
              style={{ colorScheme: 'dark' }}
            >
              <option value={0}>Unlimited</option>
              <option value={500}>500</option>
              <option value={1000}>1,000</option>
              <option value={2000}>2,000</option>
              <option value={5000}>5,000</option>
              <option value={10000}>10,000</option>
              <option value="custom">Custom...</option>
            </select>
          </div>
          {![0, 500, 1000, 2000, 5000, 10000].includes(settings.max_items) && (
            <div className="flex items-center justify-end gap-2">
              <input
                type="number"
                min={10}
                max={100000}
                value={settings.max_items}
                onChange={(e) => {
                  const v = parseInt(e.target.value);
                  if (v >= 10) updateSetting('max_items', v);
                }}
                className="w-28 rounded-lg border border-border bg-input px-3 py-1.5 text-sm text-foreground focus:outline-none focus:ring-1 focus:ring-ring"
                placeholder="Enter number"
              />
              <span className="text-xs text-muted-foreground">clips</span>
            </div>
          )}
          <div className="flex items-center justify-between">
            <span className="text-sm font-medium">Auto-delete after</span>
            <select
              value={[0, 7, 14, 30, 60, 90, 180, 365].includes(settings.auto_delete_days) ? settings.auto_delete_days : 'custom'}
              onChange={(e) => {
                const v = e.target.value;
                if (v === 'custom') updateSetting('auto_delete_days', 30);
                else updateSetting('auto_delete_days', parseInt(v));
              }}
              className="rounded-lg border border-border bg-input px-3 py-1.5 text-sm text-foreground focus:outline-none focus:ring-1 focus:ring-ring"
              style={{ colorScheme: 'dark' }}
            >
              <option value={0}>Never</option>
              <option value={7}>7 days</option>
              <option value={14}>14 days</option>
              <option value={30}>30 days</option>
              <option value={60}>60 days</option>
              <option value={90}>90 days</option>
              <option value={180}>6 months</option>
              <option value={365}>1 year</option>
              <option value="custom">Custom...</option>
            </select>
          </div>
          {![0, 7, 14, 30, 60, 90, 180, 365].includes(settings.auto_delete_days) && (
            <div className="flex items-center justify-end gap-2">
              <input
                type="number"
                min={1}
                max={3650}
                value={settings.auto_delete_days}
                onChange={(e) => {
                  const v = parseInt(e.target.value);
                  if (v >= 1) updateSetting('auto_delete_days', v);
                }}
                className="w-28 rounded-lg border border-border bg-input px-3 py-1.5 text-sm text-foreground focus:outline-none focus:ring-1 focus:ring-ring"
                placeholder="Enter days"
              />
              <span className="text-xs text-muted-foreground">days</span>
            </div>
          )}
          <p className="text-xs text-muted-foreground">
            Only applies to clips not in folders and not pinned.
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
  );
}
