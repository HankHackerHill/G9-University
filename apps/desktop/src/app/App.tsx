import { RouterProvider, createHashRouter } from 'react-router-dom';
import type { IpcClient } from '../ipc/client';
import { IpcProvider } from './ipc-context';
import { routes } from './router';

export interface AppProps {
  client: IpcClient;
}

/** Hash routing: the Tauri webview serves `dist/index.html` from an app-local origin. */
export function App({ client }: AppProps) {
  const router = createHashRouter(routes);
  return (
    <IpcProvider client={client}>
      <RouterProvider router={router} />
    </IpcProvider>
  );
}
