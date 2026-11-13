// First import, on purpose: theme.css declares the order of the cascade layers, and the first
// stylesheet to name a layer fixes its rank (see app/theme/layer-order.test.ts).
import './app/theme/theme.css';
import { StrictMode } from 'react';
import { createRoot } from 'react-dom/client';
import { App } from './app/App';
import { createDefaultClient } from './ipc';
import { applyContrast, readStoredContrast } from './app/theme/useContrast';
import { applyTheme, readStoredTheme } from './app/theme/useTheme';
import { applySkin, skinById } from './app/theme/skins';
import { readStoredSkin } from './app/theme/useSkin';

const container = document.getElementById('root');
if (container === null) {
  throw new Error('root element missing');
}

// Appearance is applied before the first render so the window never paints in the wrong scheme.
// (No inline script in index.html: the content security policy allows scripts from 'self' only.)
const stored = typeof localStorage === 'undefined' ? null : localStorage;
applyTheme(readStoredTheme(stored));
applyContrast(readStoredContrast(stored));
applySkin(skinById(readStoredSkin(stored)), readStoredContrast(stored));

createRoot(container).render(
  <StrictMode>
    <App client={createDefaultClient()} />
  </StrictMode>,
);
