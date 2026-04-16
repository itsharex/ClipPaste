import { useEffect, useCallback } from 'react';
import { WebviewWindow } from '@tauri-apps/api/webviewWindow';
import { currentMonitor } from '@tauri-apps/api/window';

const COLLAPSED_WIDTH = 14;
const COLLAPSED_HEIGHT = 80;

export function useScratchpad() {
  // Auto-create scratchpad window shortly after mount
  useEffect(() => {
    const timer = setTimeout(async () => {
      try {
        const existing = await WebviewWindow.getByLabel('scratchpad');
        if (existing) return;

        let x = 0;
        let y = 400;
        try {
          const monitor = await currentMonitor();
          if (monitor) {
            const scale = monitor.scaleFactor;
            const workW = monitor.size.width / scale;
            const workH = monitor.size.height / scale;
            const workX = monitor.position.x / scale;
            const workY = monitor.position.y / scale;
            x = workX + workW - COLLAPSED_WIDTH;
            y = workY + Math.round((workH - COLLAPSED_HEIGHT) / 2);
          }
        } catch {}

        new WebviewWindow('scratchpad', {
          url: 'index.html?window=scratchpad',
          title: 'Scratchpad',
          width: COLLAPSED_WIDTH,
          height: COLLAPSED_HEIGHT,
          x,
          y,
          resizable: false,
          decorations: false,
          alwaysOnTop: true,
          skipTaskbar: true,
          focus: false,
        });
      } catch (e) {
        console.error('Failed to create scratchpad window:', e);
      }
    }, 2000);
    return () => clearTimeout(timer);
  }, []);

  // Toggle: focus or show/hide
  const toggle = useCallback(async () => {
    const win = await WebviewWindow.getByLabel('scratchpad');
    if (win) {
      const visible = await win.isVisible();
      if (visible) {
        await win.hide();
      } else {
        await win.show();
        await win.setFocus();
      }
    }
  }, []);

  return { toggle };
}
