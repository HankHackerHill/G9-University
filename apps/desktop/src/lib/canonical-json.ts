/**
 * Canonical JSON: object keys sorted lexicographically at every level, no whitespace,
 * arrays in order. Used to compute operation parameter hashes that approvals bind to.
 * Non-finite numbers and `undefined` are rejected (they are not JSON).
 */
import type { JsonValue } from '../ipc/types';

export function canonicalJson(value: unknown): string {
  return serialize(toJson(value));
}

function toJson(value: unknown): JsonValue {
  if (value === null) {
    return null;
  }
  switch (typeof value) {
    case 'string':
    case 'boolean':
      return value;
    case 'number':
      if (!Number.isFinite(value)) {
        throw new TypeError('canonical JSON: non-finite number');
      }
      return value;
    case 'object': {
      if (Array.isArray(value)) {
        return value.map((v: unknown) => toJson(v));
      }
      const out: Record<string, JsonValue> = {};
      for (const [k, v] of Object.entries(value as Record<string, unknown>)) {
        if (v !== undefined) {
          out[k] = toJson(v);
        }
      }
      return out;
    }
    case 'undefined':
    case 'function':
    case 'symbol':
    case 'bigint':
      throw new TypeError(`canonical JSON: unsupported value of type ${typeof value}`);
    default:
      throw new TypeError('canonical JSON: unsupported value');
  }
}

function serialize(value: JsonValue): string {
  if (value === null || typeof value !== 'object') {
    return JSON.stringify(value);
  }
  if (Array.isArray(value)) {
    return `[${value.map(serialize).join(',')}]`;
  }
  const keys = Object.keys(value).sort();
  const parts: string[] = [];
  for (const key of keys) {
    const v = value[key];
    if (v !== undefined) {
      parts.push(`${JSON.stringify(key)}:${serialize(v)}`);
    }
  }
  return `{${parts.join(',')}}`;
}
