import { createContext, useContext, type ReactNode } from 'react';
import type { IpcClient } from '../ipc/client';

const IpcContext = createContext<IpcClient | null>(null);

export function IpcProvider({ client, children }: { client: IpcClient; children: ReactNode }) {
  return <IpcContext.Provider value={client}>{children}</IpcContext.Provider>;
}

export function useIpc(): IpcClient {
  const client = useContext(IpcContext);
  if (client === null) {
    throw new Error('useIpc must be used inside <IpcProvider>');
  }
  return client;
}
