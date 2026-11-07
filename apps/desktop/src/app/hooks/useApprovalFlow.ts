/**
 * Drives the APPROVAL_REQUIRED protocol from the UI side:
 *   run(invoke) → backend answers APPROVAL_REQUIRED(approval_id) → show the request →
 *   operator decides (approvals.decide) → re-run the SAME operation with approval_id.
 *
 * The backend binds the approval to the operation hash and consumes it once (ADR-0003); the
 * mock mirrors that behaviour. The native confirmation raised from Rust (Addendum A.7,
 * ADR-0014) is PROPOSED and NOT IMPLEMENTED (SEC-03 OPEN): in this build `decide` grants the
 * approval directly from the webview.
 */
import { useCallback, useState } from 'react';
import type { InvokeResult } from '../../ipc/client';
import { isG9Error } from '../../ipc/client';
import type { ApprovalRequest } from '../../ipc/operations';
import type { OperationName } from '../../ipc/operations';
import type { G9Error } from '../../ipc/types';
import { warningsOf } from '../../lib/warnings';
import { useIpc } from '../ipc-context';

export type ApprovalInvoke<K extends OperationName> = (approvalId: string | undefined) => Promise<InvokeResult<K>>;

export interface ApprovalFlow<K extends OperationName> {
  pending: boolean;
  approval: ApprovalRequest | null;
  error: G9Error | null;
  result: InvokeResult<K> | null;
  /** `CommandOutcome.warnings` of a successful `result`; empty otherwise. */
  warnings: readonly string[];
  run: (invoke: ApprovalInvoke<K>) => Promise<void>;
  decide: (decision: 'APPROVE' | 'REJECT') => Promise<void>;
  applyWithApproval: () => Promise<void>;
  reset: () => void;
}

export function useApprovalFlow<K extends OperationName>(): ApprovalFlow<K> {
  const client = useIpc();
  const [pending, setPending] = useState(false);
  const [approval, setApproval] = useState<ApprovalRequest | null>(null);
  const [error, setError] = useState<G9Error | null>(null);
  const [result, setResult] = useState<InvokeResult<K> | null>(null);
  const [invoker, setInvoker] = useState<ApprovalInvoke<K> | null>(null);

  const refreshApproval = useCallback(
    async (approvalId: string): Promise<void> => {
      const list = await client.invoke('approvals.list', {});
      if (isG9Error(list)) {
        setError(list);
        return;
      }
      setApproval(list.result.approvals.find((a) => a.approval_id === approvalId) ?? null);
    },
    [client],
  );

  const run = useCallback(
    async (invoke: ApprovalInvoke<K>): Promise<void> => {
      setPending(true);
      setError(null);
      setResult(null);
      setApproval(null);
      setInvoker(() => invoke);
      const r = await invoke(undefined);
      setResult(r);
      if (isG9Error(r)) {
        setError(r);
        if (r.category === 'APPROVAL_REQUIRED' && r.approval_id !== undefined) {
          await refreshApproval(r.approval_id);
        }
      }
      setPending(false);
    },
    [refreshApproval],
  );

  const decide = useCallback(
    async (decision: 'APPROVE' | 'REJECT'): Promise<void> => {
      if (approval === null) {
        return;
      }
      setPending(true);
      const r = await client.invoke('approvals.decide', { approval_id: approval.approval_id, decision });
      if (isG9Error(r)) {
        setError(r);
      } else {
        setError(null);
        setApproval(r.result);
      }
      setPending(false);
    },
    [approval, client],
  );

  const applyWithApproval = useCallback(async (): Promise<void> => {
    if (approval === null || invoker === null) {
      return;
    }
    setPending(true);
    const r = await invoker(approval.approval_id);
    setResult(r);
    if (isG9Error(r)) {
      setError(r);
    } else {
      setError(null);
    }
    await refreshApproval(approval.approval_id);
    setPending(false);
  }, [approval, invoker, refreshApproval]);

  const reset = useCallback(() => {
    setPending(false);
    setApproval(null);
    setError(null);
    setResult(null);
    setInvoker(null);
  }, []);

  return { pending, approval, error, result, warnings: warningsOf(result), run, decide, applyWithApproval, reset };
}
