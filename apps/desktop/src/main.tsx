import { StrictMode } from 'react';
import { createRoot } from 'react-dom/client';
import { App } from './app/App';
import { createDefaultClient } from './ipc';
import './app/theme/theme.css';

const container = document.getElementById('root');
if (container === null) {
  throw new Error('root element missing');
}

createRoot(container).render(
  <StrictMode>
    <App client={createDefaultClient()} />
  </StrictMode>,
);
