import { render, type RenderResult } from '@testing-library/react';
import type { ReactElement } from 'react';
import { MemoryRouter } from 'react-router-dom';
import { IpcProvider } from '../app/ipc-context';
import { createClient, type IpcClient } from '../ipc/client';
import { createMockTransport, type MockOptions, type MockTransport } from '../ipc/transports/mock';

export interface Harness {
  transport: MockTransport;
  client: IpcClient;
}

let counter = 0;

/** Deterministic client: fixed clock from the mock, sequential ids instead of crypto.randomUUID. */
export function createHarness(options: MockOptions = {}): Harness {
  const transport = createMockTransport(options);
  const client = createClient(transport, {
    now: () => transport.clock.now(),
    uuid: () => {
      counter += 1;
      return `test-uuid-${counter}`;
    },
  });
  return { transport, client };
}

export function renderWithIpc(ui: ReactElement, harness: Harness = createHarness(), route = '/'): RenderResult & Harness {
  const result = render(
    <IpcProvider client={harness.client}>
      <MemoryRouter initialEntries={[route]}>{ui}</MemoryRouter>
    </IpcProvider>,
  );
  return { ...result, ...harness };
}
