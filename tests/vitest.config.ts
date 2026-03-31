import { defineConfig } from 'vitest/config';
import path from 'path';

export default defineConfig({
  resolve: {
    alias: {
      '@': path.resolve(__dirname, '../frontend/src'),
      // Deduplicate React — force all imports to use the tests/ copy
      'react': path.resolve(__dirname, 'node_modules/react'),
      'react-dom': path.resolve(__dirname, 'node_modules/react-dom'),
      'react/jsx-runtime': path.resolve(__dirname, 'node_modules/react/jsx-runtime'),
      'react/jsx-dev-runtime': path.resolve(__dirname, 'node_modules/react/jsx-dev-runtime'),
      '@tanstack/react-virtual': path.resolve(__dirname, 'node_modules/@tanstack/react-virtual'),
      // Mock Tauri APIs for testing
      '@tauri-apps/api/core': path.resolve(__dirname, '__mocks__/tauri-api-core.ts'),
      '@tauri-apps/api/event': path.resolve(__dirname, '__mocks__/tauri-api-event.ts'),
      '@tauri-apps/api/window': path.resolve(__dirname, '__mocks__/tauri-api-window.ts'),
      '@tauri-apps/api/webviewWindow': path.resolve(__dirname, '__mocks__/tauri-api-webviewWindow.ts'),
      '@tauri-apps/plugin-clipboard-manager': path.resolve(__dirname, '__mocks__/tauri-plugin-clipboard.ts'),
      '@tauri-apps/plugin-updater': path.resolve(__dirname, '__mocks__/tauri-plugin-updater.ts'),
      '@tauri-apps/plugin-process': path.resolve(__dirname, '__mocks__/tauri-plugin-process.ts'),
      '@tauri-apps/plugin-opener': path.resolve(__dirname, '__mocks__/tauri-plugin-opener.ts'),
      '@tauri-apps/plugin-log': path.resolve(__dirname, '__mocks__/tauri-plugin-log.ts'),
      'sonner': path.resolve(__dirname, '__mocks__/sonner.ts'),
      'use-shortcut-recorder': path.resolve(__dirname, '__mocks__/use-shortcut-recorder.ts'),
    },
  },
  test: {
    globals: true,
    environment: 'jsdom',
    setupFiles: ['./setup.ts'],
    include: ['**/*.test.{ts,tsx}'],
    css: false,
    // Point to the actual source
    root: '.',
  },
});
