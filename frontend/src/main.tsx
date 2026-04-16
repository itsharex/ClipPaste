import ReactDOM from 'react-dom/client';
import App from './App';
import { SettingsWindow } from './windows/SettingsWindow';
import { ScratchpadWindow } from './windows/ScratchpadWindow';
import { ErrorBoundary } from './components/ErrorBoundary';
import { attachConsole } from '@tauri-apps/plugin-log';
import './index.css';

attachConsole().catch((err) => console.error('[ClipPaste] Failed to attach Tauri console:', err));

const urlParams = new URLSearchParams(window.location.search);
const windowType = urlParams.get('window');

ReactDOM.createRoot(document.getElementById('root')!).render(
  <ErrorBoundary>
    {windowType === 'settings' ? <SettingsWindow /> : windowType === 'scratchpad' ? <ScratchpadWindow /> : <App />}
  </ErrorBoundary>
);
