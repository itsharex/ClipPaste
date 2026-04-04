import { useEffect, useRef, useState } from 'react';
import { ClipboardItem } from '../types';

interface EditClipModalProps {
  clip: ClipboardItem | null;
  onPaste: (editedText: string) => void;
  onClose: () => void;
}

export function EditClipModal({ clip, onPaste, onClose }: EditClipModalProps) {
  const [text, setText] = useState('');
  const textareaRef = useRef<HTMLTextAreaElement>(null);
  const textRef = useRef(text);
  textRef.current = text;
  const onPasteRef = useRef(onPaste);
  onPasteRef.current = onPaste;
  const onCloseRef = useRef(onClose);
  onCloseRef.current = onClose;

  useEffect(() => {
    if (clip) {
      setText(clip.content);
      setTimeout(() => {
        textareaRef.current?.focus();
        textareaRef.current?.select();
      }, 50);
    }
  }, [clip]);

  useEffect(() => {
    if (!clip) return;
    const handleKeyDown = (e: KeyboardEvent) => {
      if (e.key === 'Escape') {
        e.preventDefault();
        onCloseRef.current();
      }
      if (e.key === 'Enter' && !e.shiftKey) {
        e.preventDefault();
        onPasteRef.current(textRef.current);
      }
    };
    document.addEventListener('keydown', handleKeyDown);
    return () => document.removeEventListener('keydown', handleKeyDown);
  }, [clip]);

  if (!clip) return null;

  return (
    <div
      className="absolute inset-0 z-50 flex items-center justify-center"
      style={{ backgroundColor: 'rgba(0,0,0,0.5)' }} /* bg-black/50 */
      onMouseDown={(e) => { if (e.target === e.currentTarget) onClose(); }}
    >
      <div className="flex w-[90%] flex-col gap-2 rounded-lg border border-border bg-popover p-3 shadow-xl">
        <p className="text-xs text-muted-foreground">Edit before paste · Enter to paste · Shift+Enter for new line · Esc to cancel</p>
        <textarea
          ref={textareaRef}
          value={text}
          onChange={(e) => setText(e.target.value)}
          className="h-28 w-full resize-none rounded-md border border-border bg-background px-2 py-1.5 text-sm text-foreground outline-none focus:ring-1 focus:ring-ring"
        />
        <div className="flex justify-end gap-2">
          <button
            onClick={onClose}
            className="rounded-md px-3 py-1 text-sm text-muted-foreground hover:bg-accent"
          >
            Cancel
          </button>
          <button
            onClick={() => onPaste(text)}
            className="rounded-md bg-primary px-3 py-1 text-sm text-primary-foreground hover:opacity-90"
          >
            Paste
          </button>
        </div>
      </div>
    </div>
  );
}
