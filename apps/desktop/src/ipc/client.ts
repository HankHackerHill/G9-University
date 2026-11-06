/**
 * Typed IPC client. Builds the wire-level `ClientCommandMeta` (never an actor: ADR-0013,
 * Addendum A.8), hands it to a transport and narrows the raw result to
 * `CommandOutcome<T> | G9Error`. Anything that is neither is reported as an INTERNAL error
 * with code `ipc.malformed_response` — the UI never trusts an unshaped payload.
 */
import type { OperationName, RequestOf, ResponseOf } from './operations';
import { OPERATIONS } from './operations';
import type {
  ClientCommandMeta,
  CommandOutcome,
  DataDomain,
  ErrorCategory,
  G9Error,
  Id,
  InteractionMode,
  JsonValue,
  ResourceRef,
  Revision,
} from './types';
import { ERROR_CATEGORIES } from './types';

export interface IpcTransport {
  readonly kind: 'mock' | 'tauri';
  invoke(op: OperationName, meta: ClientCommandMeta, payload: unknown): Promise<unknown>;
}

export interface InvokeOptions {
  /** Overrides the operation's default privacy domain. */
  domain?: DataDomain;
  /** Overrides the operation's default interaction mode. */
  mode?: InteractionMode;
  resource?: ResourceRef;
  expectedRevision?: Revision;
  approvalId?: Id;
  /** Reuse a key to replay an earlier command (idempotency store). */
  idempotencyKey?: Id;
  traceId?: Id;
}

export type InvokeResult<K extends OperationName> = CommandOutcome<ResponseOf<K>> | G9Error;

export interface IpcClient {
  readonly transportKind: 'mock' | 'tauri';
  invoke<K extends OperationName>(op: K, payload: RequestOf<K>, options?: InvokeOptions): Promise<InvokeResult<K>>;
}

export interface ClientDeps {
  now: () => number;
  uuid: () => string;
}

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === 'object' && value !== null && !Array.isArray(value);
}

export function isErrorCategory(value: unknown): value is ErrorCategory {
  return typeof value === 'string' && (ERROR_CATEGORIES as readonly string[]).includes(value);
}

/** Type guard: a `G9Error` has exactly the closed category set and string code/message. */
export function isG9Error(value: unknown): value is G9Error {
  if (!isRecord(value)) {
    return false;
  }
  if (!isErrorCategory(value.category)) {
    return false;
  }
  if (typeof value.code !== 'string' || typeof value.message !== 'string') {
    return false;
  }
  if ('approval_id' in value && value.approval_id !== undefined && typeof value.approval_id !== 'string') {
    return false;
  }
  return true;
}

/** Type guard for the success envelope shape (result present, replayed boolean). */
export function isCommandOutcome(value: unknown): value is CommandOutcome<unknown> {
  if (!isRecord(value)) {
    return false;
  }
  if (!('result' in value)) {
    return false;
  }
  if (typeof value.replayed !== 'boolean') {
    return false;
  }
  if ('revision' in value && value.revision !== undefined && typeof value.revision !== 'number') {
    return false;
  }
  if ('warnings' in value && value.warnings !== undefined) {
    if (!Array.isArray(value.warnings) || !value.warnings.every((w) => typeof w === 'string')) {
      return false;
    }
  }
  return true;
}

export function malformedResponse(op: string, details?: JsonValue): G9Error {
  const error: G9Error = {
    category: 'INTERNAL',
    code: 'ipc.malformed_response',
    message: `the backend returned an unrecognised envelope for ${op}`,
  };
  if (details !== undefined) {
    error.details = details;
  }
  return error;
}

export function transportFailure(op: string, cause: unknown): G9Error {
  // The cause may carry anything (stack traces, paths). Only its kind is surfaced.
  const kind = cause instanceof Error ? cause.name : typeof cause;
  return {
    category: 'RESOURCE_UNAVAILABLE',
    code: 'ipc.transport_failed',
    message: `the IPC transport failed while invoking ${op}`,
    details: { cause_kind: kind },
  };
}

const defaultDeps: ClientDeps = {
  now: () => Date.now(),
  uuid: () => crypto.randomUUID(),
};

export function buildMeta(op: OperationName, options: InvokeOptions, deps: ClientDeps): ClientCommandMeta {
  const spec = OPERATIONS[op];
  const meta: ClientCommandMeta = {
    request_id: deps.uuid(),
    operation: op,
    idempotency_key: options.idempotencyKey ?? deps.uuid(),
    scope: { domain: options.domain ?? spec.defaultDomain, mode: options.mode ?? spec.defaultMode },
    payload_schema_version: spec.schemaVersion,
    trace_id: options.traceId ?? deps.uuid(),
    issued_at: deps.now(),
  };
  if (options.resource !== undefined) {
    meta.resource = options.resource;
  }
  if (options.expectedRevision !== undefined) {
    meta.expected_revision = options.expectedRevision;
  }
  if (options.approvalId !== undefined) {
    meta.approval_id = options.approvalId;
  }
  return meta;
}

export function createClient(transport: IpcTransport, deps: ClientDeps = defaultDeps): IpcClient {
  return {
    transportKind: transport.kind,
    async invoke<K extends OperationName>(op: K, payload: RequestOf<K>, options: InvokeOptions = {}): Promise<InvokeResult<K>> {
      const meta = buildMeta(op, options, deps);
      let raw: unknown;
      try {
        raw = await transport.invoke(op, meta, payload);
      } catch (cause: unknown) {
        if (isG9Error(cause)) {
          return cause;
        }
        return transportFailure(op, cause);
      }
      if (isG9Error(raw)) {
        return raw;
      }
      if (isCommandOutcome(raw)) {
        // The response payload shape is the backend's responsibility (schema-checked there);
        // the client narrows the envelope only.
        return raw as CommandOutcome<ResponseOf<K>>;
      }
      return malformedResponse(op, { received_type: Array.isArray(raw) ? 'array' : typeof raw });
    },
  };
}
