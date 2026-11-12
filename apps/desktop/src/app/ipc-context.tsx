import { createContext, useContext, useMemo, useSyncExternalStore, type ReactNode } from 'react';
import type { InvokeOptions, InvokeResult, IpcClient } from '../ipc/client';
import { OPERATIONS, type OperationName, type RequestOf } from '../ipc/operations';

/**
 * Counts mutating commands issued through this provider. Shell furniture that summarises
 * backend state (the day rail, the lamps) re-reads when it changes, so a session started on
 * one screen shows on the rail without polling and without the window keeping its own clock.
 */
interface Epoch {
  get: () => number;
  subscribe: (notify: () => void) => () => void;
  bump: () => void;
}

function createEpoch(): Epoch {
  let value = 0;
  const listeners = new Set<() => void>();
  return {
    get: () => value,
    subscribe: (notify) => {
      listeners.add(notify);
      return () => {
        listeners.delete(notify);
      };
    },
    bump: () => {
      value += 1;
      for (const notify of listeners) {
        notify();
      }
    },
  };
}

const IpcContext = createContext<IpcClient | null>(null);
const EpochContext = createContext<Epoch | null>(null);

export function IpcProvider({ client, children }: { client: IpcClient; children: ReactNode }) {
  const value = useMemo(() => {
    const epoch = createEpoch();
    const counted: IpcClient = {
      transportKind: client.transportKind,
      async invoke<K extends OperationName>(op: K, payload: RequestOf<K>, options?: InvokeOptions): Promise<InvokeResult<K>> {
        const result = await client.invoke(op, payload, options);
        // A refused command can still change what the shell shows (an approval request is
        // created by an APPROVAL_REQUIRED answer), so every mutating call counts.
        if (OPERATIONS[op].mutating) {
          epoch.bump();
        }
        return result;
      },
    };
    return { client: counted, epoch };
  }, [client]);

  return (
    <IpcContext.Provider value={value.client}>
      <EpochContext.Provider value={value.epoch}>{children}</EpochContext.Provider>
    </IpcContext.Provider>
  );
}

export function useIpc(): IpcClient {
  const client = useContext(IpcContext);
  if (client === null) {
    throw new Error('useIpc must be used inside <IpcProvider>');
  }
  return client;
}

const NO_EPOCH: Epoch = { get: () => 0, subscribe: () => () => undefined, bump: () => undefined };

/** Number of mutating commands issued so far; changes after each one returns. */
export function useMutationEpoch(): number {
  const epoch = useContext(EpochContext) ?? NO_EPOCH;
  return useSyncExternalStore(epoch.subscribe, epoch.get, epoch.get);
}
