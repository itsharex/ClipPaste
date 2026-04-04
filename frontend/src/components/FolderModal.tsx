import { useEffect, useRef, useState } from 'react';
import { createPortal } from 'react-dom';
import { clsx } from 'clsx';
import {
  Briefcase, Code, Bookmark, Palette, Lock,
  Star, Heart, Zap, Coffee, Music,
  Globe, Camera, Gamepad2, Rocket, ShoppingBag,
  GraduationCap, Wrench, Lightbulb, MessageSquare, Flame,
  // Tech / DevOps
  Database, Server, Container, Network, HardDrive,
  Terminal, Shield, Cpu, Cloud, Leaf,
  Ship, Anchor, Rabbit, Bug, Key, Fish,
  // Extra DevOps / Hardware
  Laptop2, Monitor, PcCase, Wifi, Router,
  GitBranch, Github, Package, Workflow, Gauge,
  Cog, Cable, Plug, Activity, Hash,
  ShieldCheck, LockKeyhole, AppWindow, RefreshCw, Blocks,
  type LucideIcon,
} from 'lucide-react';

export const FOLDER_ICON_OPTIONS: { key: string; Icon: LucideIcon; color?: string }[] = [
  // General
  { key: 'briefcase', Icon: Briefcase, color: 'text-amber-400' },
  { key: 'code', Icon: Code, color: 'text-emerald-400' },
  { key: 'bookmark', Icon: Bookmark, color: 'text-blue-400' },
  { key: 'palette', Icon: Palette, color: 'text-pink-400' },
  { key: 'lock', Icon: Lock, color: 'text-yellow-500' },
  { key: 'star', Icon: Star, color: 'text-yellow-400' },
  { key: 'heart', Icon: Heart, color: 'text-red-400' },
  { key: 'zap', Icon: Zap, color: 'text-yellow-300' },
  { key: 'coffee', Icon: Coffee, color: 'text-amber-600' },
  { key: 'music', Icon: Music, color: 'text-purple-400' },
  { key: 'globe', Icon: Globe, color: 'text-cyan-400' },
  { key: 'camera', Icon: Camera, color: 'text-slate-300' },
  { key: 'gamepad', Icon: Gamepad2, color: 'text-indigo-400' },
  { key: 'rocket', Icon: Rocket, color: 'text-orange-400' },
  { key: 'shopping', Icon: ShoppingBag, color: 'text-pink-300' },
  { key: 'graduation', Icon: GraduationCap, color: 'text-blue-300' },
  { key: 'wrench', Icon: Wrench, color: 'text-slate-400' },
  { key: 'lightbulb', Icon: Lightbulb, color: 'text-yellow-300' },
  { key: 'message', Icon: MessageSquare, color: 'text-sky-400' },
  { key: 'flame', Icon: Flame, color: 'text-orange-500' },
  // Tech / DevOps
  { key: 'database', Icon: Database, color: 'text-red-400' },
  { key: 'server', Icon: Server, color: 'text-slate-300' },
  { key: 'container', Icon: Container, color: 'text-sky-400' },
  { key: 'network', Icon: Network, color: 'text-violet-400' },
  { key: 'harddrive', Icon: HardDrive, color: 'text-zinc-400' },
  { key: 'terminal', Icon: Terminal, color: 'text-green-400' },
  { key: 'shield', Icon: Shield, color: 'text-emerald-500' },
  { key: 'cpu', Icon: Cpu, color: 'text-teal-400' },
  { key: 'cloud', Icon: Cloud, color: 'text-sky-300' },
  { key: 'leaf', Icon: Leaf, color: 'text-green-500' },
  { key: 'ship', Icon: Ship, color: 'text-blue-400' },
  { key: 'anchor', Icon: Anchor, color: 'text-blue-500' },
  { key: 'rabbit', Icon: Rabbit, color: 'text-orange-400' },
  { key: 'bug', Icon: Bug, color: 'text-red-300' },
  { key: 'key', Icon: Key, color: 'text-yellow-400' },
  { key: 'fish', Icon: Fish, color: 'text-sky-500' },
  // Hardware / Infra
  { key: 'laptop', Icon: Laptop2, color: 'text-slate-300' },
  { key: 'monitor', Icon: Monitor, color: 'text-blue-300' },
  { key: 'pc', Icon: PcCase, color: 'text-zinc-400' },
  { key: 'wifi', Icon: Wifi, color: 'text-cyan-400' },
  { key: 'router', Icon: Router, color: 'text-indigo-400' },
  // DevOps / CI
  { key: 'git', Icon: GitBranch, color: 'text-orange-500' },
  { key: 'github', Icon: Github, color: 'text-slate-200' },
  { key: 'package', Icon: Package, color: 'text-amber-400' },
  { key: 'workflow', Icon: Workflow, color: 'text-violet-400' },
  { key: 'gauge', Icon: Gauge, color: 'text-emerald-400' },
  { key: 'cog', Icon: Cog, color: 'text-slate-400' },
  { key: 'cable', Icon: Cable, color: 'text-yellow-500' },
  { key: 'plug', Icon: Plug, color: 'text-green-400' },
  { key: 'activity', Icon: Activity, color: 'text-red-400' },
  { key: 'hash', Icon: Hash, color: 'text-purple-400' },
  // SSL / Web Server
  { key: 'shieldcheck', Icon: ShieldCheck, color: 'text-green-500' },
  { key: 'lockkeyhole', Icon: LockKeyhole, color: 'text-yellow-400' },
  { key: 'appwindow', Icon: AppWindow, color: 'text-emerald-400' },
  { key: 'refresh', Icon: RefreshCw, color: 'text-blue-400' },
  { key: 'blocks', Icon: Blocks, color: 'text-indigo-400' },
];

export const FOLDER_ICON_MAP: Record<string, { Icon: LucideIcon; color: string }> = Object.fromEntries(
  FOLDER_ICON_OPTIONS.map(({ key, Icon, color }) => [key, { Icon, color: color || '' }])
);

const COLOR_OPTIONS = [
  { key: 'red', bg: 'bg-red-500' },
  { key: 'orange', bg: 'bg-orange-500' },
  { key: 'amber', bg: 'bg-amber-400' },
  { key: 'green', bg: 'bg-green-500' },
  { key: 'blue', bg: 'bg-blue-500' },
  { key: 'violet', bg: 'bg-violet-500' },
  { key: 'pink', bg: 'bg-pink-500' },
  { key: 'rose', bg: 'bg-rose-500' },
] as const;

interface FolderModalProps {
  isOpen: boolean;
  mode: 'create' | 'rename';
  initialName: string;
  initialColor?: string | null;
  initialIcon?: string | null;
  onClose: () => void;
  onSubmit: (name: string, color: string | null, icon: string | null) => void;
}

export function FolderModal({ isOpen, mode, initialName, initialColor, initialIcon, onClose, onSubmit }: FolderModalProps) {
  const inputRef = useRef<HTMLInputElement>(null);
  const [isSubmitting, setIsSubmitting] = useState(false);
  const [selectedColor, setSelectedColor] = useState<string | null>(initialColor ?? null);
  const [selectedIcon, setSelectedIcon] = useState<string | null>(initialIcon ?? null);

  useEffect(() => {
    if (isOpen) {
      setIsSubmitting(false);
      setSelectedColor(initialColor ?? null);
      setSelectedIcon(initialIcon ?? null);
      if (inputRef.current) {
        setTimeout(() => inputRef.current?.focus(), 50);
        if (mode === 'rename') {
          setTimeout(() => inputRef.current?.select(), 50);
        }
      }
    }
  }, [isOpen, mode, initialColor, initialIcon]);

  if (!isOpen) return null;

  const handleSubmit = async () => {
    if (isSubmitting) return;
    const val = inputRef.current?.value.trim();
    if (!val) return;
    if (val.length > 50 || /[<>:"|?*\\]/.test(val)) return;
    setIsSubmitting(true);
    await onSubmit(val, selectedColor, selectedIcon);
    setIsSubmitting(false);
  };

  return createPortal(
    <div className="fixed inset-0 z-[9999] overflow-y-auto bg-black/50 p-4">
      <div className="mx-auto w-80 rounded-xl border border-border bg-card p-4 shadow-2xl">
        <h3 className="mb-2 text-sm font-semibold text-foreground">
          {mode === 'create' ? 'Create New Folder' : 'Rename Folder'}
        </h3>
        <input
          ref={inputRef}
          type="text"
          placeholder="Folder Name"
          defaultValue={initialName}
          className="mb-2 w-full rounded-md border border-input bg-input px-3 py-1.5 text-sm text-foreground focus:border-primary focus:outline-none focus:ring-1 focus:ring-primary"
          onKeyDown={(e) => {
            if (e.key === 'Enter') handleSubmit();
            else if (e.key === 'Escape') onClose();
          }}
        />

        {/* Icon Picker */}
        <div className="mb-2">
          <p className="mb-1 text-xs text-muted-foreground">Icon</p>
          <div className="flex flex-wrap items-center gap-1.5">
            <button
              onClick={() => setSelectedIcon(null)}
              title="None"
              className={clsx(
                'flex h-7 w-7 items-center justify-center rounded-md border transition-all',
                selectedIcon === null
                  ? 'border-primary bg-primary/20 text-primary'
                  : 'border-transparent text-muted-foreground hover:bg-accent'
              )}
            >
              <span className="text-[10px] font-bold">Aa</span>
            </button>
            {FOLDER_ICON_OPTIONS.map(({ key, Icon, color }) => (
              <button
                key={key}
                onClick={() => setSelectedIcon(key)}
                title={key}
                className={clsx(
                  'flex h-7 w-7 items-center justify-center rounded-md border transition-all',
                  selectedIcon === key
                    ? 'border-primary bg-primary/20'
                    : 'border-transparent hover:bg-accent',
                  selectedIcon === key ? color : 'text-muted-foreground hover:text-foreground'
                )}
              >
                <Icon size={14} />
              </button>
            ))}
          </div>
        </div>

        {/* Color Picker */}
        <div className="mb-2">
          <p className="mb-1 text-xs text-muted-foreground">Color</p>
          <div className="flex items-center gap-2">
            <button
              onClick={() => setSelectedColor(null)}
              title="Auto"
              className={clsx(
                'h-5 w-5 rounded-full border-2 bg-gradient-to-br from-gray-300 to-gray-500 transition-all',
                selectedColor === null ? 'scale-125 border-white' : 'border-transparent'
              )}
            />
            {COLOR_OPTIONS.map(({ key, bg }) => (
              <button
                key={key}
                onClick={() => setSelectedColor(key)}
                title={key}
                className={clsx(
                  'h-5 w-5 rounded-full border-2 transition-all',
                  bg,
                  selectedColor === key ? 'scale-125 border-white' : 'border-transparent'
                )}
              />
            ))}
          </div>
        </div>

        <div className="flex justify-end gap-2">
          <button
            onClick={onClose}
            disabled={isSubmitting}
            className="rounded-md px-3 py-1.5 text-sm font-medium text-muted-foreground hover:bg-secondary hover:text-foreground disabled:cursor-not-allowed disabled:opacity-50"
          >
            Cancel
          </button>
          <button
            onClick={handleSubmit}
            disabled={isSubmitting}
            className="rounded-md bg-primary px-3 py-1.5 text-sm font-medium text-primary-foreground hover:bg-primary/90 disabled:cursor-not-allowed disabled:opacity-50"
          >
            {isSubmitting ? 'Saving...' : mode === 'create' ? 'Create' : 'Save'}
          </button>
        </div>
      </div>
    </div>,
    document.body
  );
}
