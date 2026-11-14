import type { InvokeResult } from '../../ipc/client';
import type { OperationName } from '../../ipc/operations';
import type { IanaZone } from '../../ipc/types';
import type { ApprovalFlow } from '../hooks/useApprovalFlow';
import { ApprovalCard, ConfirmationCancelled } from './ApprovalCard';
import { ErrorDisplay } from './ErrorDisplay';

/**
 * What a protected action answered, directly under the control that asked: the backend's
 * refusal or its request for approval, the voucher to decide and apply it, and the operator's
 * own cancellation of the native confirmation. `onApplied` runs after the approved operation
 * was re-issued, with its answer (an error or the result).
 */
export function FlowOutcome<K extends OperationName>({ flow, zone, now, onApplied }: { flow: ApprovalFlow<K>; zone: IanaZone; now: number | null; onApplied: (answer: InvokeResult<K> | null) => void }) {
  return (
    <>
      {flow.error !== null ? <ErrorDisplay error={flow.error} /> : null}
      {flow.cancelled ? <ConfirmationCancelled /> : null}
      {flow.approval !== null && flow.approval.state !== 'CONSUMED' ? (
        <ApprovalCard
          approval={flow.approval}
          zone={zone}
          now={now ?? undefined}
          disabled={flow.pending}
          onDecide={(d) => {
            void flow.decide(d);
          }}
          onApply={() => {
            void flow.applyWithApproval().then(onApplied);
          }}
        />
      ) : null}
    </>
  );
}
