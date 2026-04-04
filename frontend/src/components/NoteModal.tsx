import { useEffect, useRef, useState } from 'react';

interface NoteModalProps {
  isOpen: boolean;
  clipId: string | null;
  initialNote: string;
  onSave: (clipId: string, note: string | null) => void;
  onClose: () => void;
}

export function NoteModal({ isOpen, clipId, initialNote, onSave, onClose }: NoteModalProps) {
  const [text, setText] = useState('');
  const inputRef = useRef<HTMLInputElement>(null);
  const textRef = useRef(text);
  textRef.current = text;
  const onSaveRef = useRef(onSave);
  onSaveRef.current = onSave;
  const onCloseRef = useRef(onClose);
  onCloseRef.current = onClose;
  const clipIdRef = useRef(clipId);
  clipIdRef.current = clipId;

  useEffect(() => {
    if (isOpen) {
      setText(initialNote);
      setTimeout(() => {
        inputRef.current?.focus();
        inputRef.current?.select();
      }, 50);
    }
  }, [isOpen, initialNote]);

  useEffect(() => {
    if (!isOpen) return;
    const handleKeyDown = (e: KeyboardEvent) => {
      if (e.key === 'Escape') { e.preventDefault(); onCloseRef.current(); }
      if (e.key === 'Enter') {
        e.preventDefault();
        if (clipIdRef.current) {
          onSaveRef.current(clipIdRef.current, textRef.current.trim() || null);
        }
      }
    };
    document.addEventListener('keydown', handleKeyDown);
    return () => document.removeEventListener('keydown', handleKeyDown);
  }, [isOpen]);

  const handleSave = () => {
    if (!clipId) return;
    onSave(clipId, text.trim() || null);
  };

  if (!isOpen || !clipId) return null;

  return (
    <div
      className="absolute inset-0 z-50 flex items-center justify-center"
      style={{ backgroundColor: 'rgba(0,0,0,0.5)' }} /* bg-black/50 */
      onMouseDown={(e) => { if (e.target === e.currentTarget) onClose(); }}
    >
      <div className="flex w-[80%] flex-col gap-2 rounded-lg border border-border bg-popover p-3 shadow-xl">
        <p className="text-xs text-muted-foreground">Note · Enter to save · Esc to cancel</p>
        <input
          ref={inputRef}
          value={text}
          onChange={(e) => setText(e.target.value)}
          placeholder="Add a note..."
          className="w-full rounded-md border border-border bg-background px-2 py-1.5 text-sm text-foreground outline-none focus:ring-1 focus:ring-ring"
        />
        <div className="flex justify-end gap-2">
          {initialNote && (
            <button
              onClick={() => { if (clipId) onSave(clipId, null); }}
              className="rounded-md px-3 py-1 text-sm text-red-400 hover:bg-red-500/10"
            >
              Remove
            </button>
          )}
          <div className="flex-1" />
          <button
            onClick={onClose}
            className="rounded-md px-3 py-1 text-sm text-muted-foreground hover:bg-accent"
          >
            Cancel
          </button>
          <button
            onClick={handleSave}
            className="rounded-md bg-primary px-3 py-1 text-sm text-primary-foreground hover:opacity-90"
          >
            Save
          </button>
        </div>
      </div>
    </div>
  );
}
