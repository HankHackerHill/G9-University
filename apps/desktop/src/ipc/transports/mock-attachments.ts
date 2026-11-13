/**
 * Port of the checks `evidence.attach_file` runs, for the mock transport only:
 *  - crates/g9-app/src/commands/evidence.rs `clean_filename`, `decode_bytes`;
 *  - crates/g9-evidence/src/attachments.rs `AttachmentRef::new`, `refused_signature`,
 *    `MediaType::sniff`, `MediaType::consistent_with`.
 * The backend is authoritative; these functions only let the mock answer with the same codes.
 * Everything is synchronous and deterministic (no FileReader, no crypto.subtle).
 */
import { MAX_ATTACHMENT_BYTES } from '../../lib/attachment';
import type { AttachmentMediaType } from '../operations';

/** Longest original file name kept, in UTF-8 bytes (evidence.rs `MAX_FILENAME_BYTES`). */
export const MAX_FILENAME_BYTES = 255;

const BASE64_ALPHABET = 'ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/';
const BASE64_VALUE: Int16Array = (() => {
  const table = new Int16Array(128).fill(-1);
  for (let i = 0; i < BASE64_ALPHABET.length; i += 1) {
    table[BASE64_ALPHABET.charCodeAt(i)] = i;
  }
  return table;
})();
const PAD = 0x3d; // '='

function base64Value(code: number): number {
  return code < 128 ? (BASE64_VALUE[code] ?? -1) : -1;
}

/**
 * Standard-alphabet, padded, canonical base64 (the `base64` crate's `general_purpose::STANDARD`
 * engine): length a multiple of 4, `=` only as final padding, no whitespace, and the unused bits
 * of the last symbol zero. Returns null when `text` is not such a string.
 */
export function decodeBase64Strict(text: string): Uint8Array | null {
  const n = text.length;
  if (n % 4 !== 0) {
    return null;
  }
  let pad = 0;
  if (n > 0 && text.charCodeAt(n - 1) === PAD) {
    pad = text.charCodeAt(n - 2) === PAD ? 2 : 1;
  }
  const out = new Uint8Array((n / 4) * 3 - pad);
  let o = 0;
  for (let i = 0; i < n; i += 4) {
    const last = i + 4 === n;
    const a = base64Value(text.charCodeAt(i));
    const b = base64Value(text.charCodeAt(i + 1));
    const c = last && pad === 2 ? 0 : base64Value(text.charCodeAt(i + 2));
    const d = last && pad >= 1 ? 0 : base64Value(text.charCodeAt(i + 3));
    if (a < 0 || b < 0 || c < 0 || d < 0) {
      return null;
    }
    if (last && pad === 2 && (b & 0x0f) !== 0) {
      return null;
    }
    if (last && pad === 1 && (c & 0x03) !== 0) {
      return null;
    }
    const triple = (a << 18) | (b << 12) | (c << 6) | d;
    out[o] = (triple >> 16) & 0xff;
    o += 1;
    if (!(last && pad === 2)) {
      out[o] = (triple >> 8) & 0xff;
      o += 1;
    }
    if (!(last && pad >= 1)) {
      out[o] = triple & 0xff;
      o += 1;
    }
  }
  return out;
}

/** Standard padded base64 of `bytes` (test fixtures and the UI build payloads with it). */
export function encodeBase64(bytes: Uint8Array): string {
  let out = '';
  for (let i = 0; i < bytes.length; i += 3) {
    const a = bytes[i] ?? 0;
    const b = bytes[i + 1] ?? 0;
    const c = bytes[i + 2] ?? 0;
    const triple = (a << 16) | (b << 8) | c;
    out += BASE64_ALPHABET.charAt((triple >> 18) & 0x3f);
    out += BASE64_ALPHABET.charAt((triple >> 12) & 0x3f);
    out += i + 1 < bytes.length ? BASE64_ALPHABET.charAt((triple >> 6) & 0x3f) : '=';
    out += i + 2 < bytes.length ? BASE64_ALPHABET.charAt(triple & 0x3f) : '=';
  }
  return out;
}

/** Unicode `White_Space` (what Rust's `str::trim` removes; JavaScript's `trim` differs on U+0085 and U+FEFF). */
function isRustWhitespace(code: number): boolean {
  return (
    (code >= 0x09 && code <= 0x0d) ||
    code === 0x20 ||
    code === 0x85 ||
    code === 0xa0 ||
    code === 0x1680 ||
    (code >= 0x2000 && code <= 0x200a) ||
    code === 0x2028 ||
    code === 0x2029 ||
    code === 0x202f ||
    code === 0x205f ||
    code === 0x3000
  );
}

/** Rust `str::trim`: strips leading and trailing Unicode `White_Space`. */
export function rustTrim(text: string): string {
  let start = 0;
  let end = text.length;
  while (start < end && isRustWhitespace(text.charCodeAt(start))) {
    start += 1;
  }
  while (end > start && isRustWhitespace(text.charCodeAt(end - 1))) {
    end -= 1;
  }
  return text.slice(start, end);
}

/**
 * evidence.rs `clean_filename`: the trimmed name, or null when it is empty, longer than 255
 * UTF-8 bytes, contains a control character (Unicode Cc), `/` or `\`, or consists of dots only.
 */
export function cleanFilename(name: string): string | null {
  const trimmed = rustTrim(name);
  if (trimmed.length === 0 || new TextEncoder().encode(trimmed).length > MAX_FILENAME_BYTES) {
    return null;
  }
  for (let i = 0; i < trimmed.length; i += 1) {
    const code = trimmed.charCodeAt(i);
    if (code <= 0x1f || (code >= 0x7f && code <= 0x9f) || code === 0x2f || code === 0x5c) {
      return null;
    }
  }
  return /^\.+$/.test(trimmed) ? null : trimmed;
}

export type DecodedContent = { ok: true; bytes: Uint8Array } | { ok: false; code: 'attachment.too_large' | 'attachment.content_invalid' };

/** evidence.rs `decode_bytes`: refuse an oversized payload before decoding, then decode strictly, then check the decoded size. */
export function decodeContent(content: string): DecodedContent {
  if (Math.floor(content.length / 4) * 3 > MAX_ATTACHMENT_BYTES + 3) {
    return { ok: false, code: 'attachment.too_large' };
  }
  const bytes = decodeBase64Strict(content);
  if (bytes === null) {
    return { ok: false, code: 'attachment.content_invalid' };
  }
  if (bytes.length > MAX_ATTACHMENT_BYTES) {
    return { ok: false, code: 'attachment.too_large' };
  }
  return { ok: true, bytes };
}

function startsWith(bytes: Uint8Array, prefix: readonly number[]): boolean {
  return prefix.length <= bytes.length && prefix.every((b, i) => bytes[i] === b);
}

/** g9-evidence `refused_signature`: content families refused whatever the declared type. */
const REFUSED_SIGNATURES: readonly (readonly [readonly number[], string])[] = [
  [[0x7f, 0x45, 0x4c, 0x46], 'ELF executable'],
  [[0x4d, 0x5a], 'PE/DOS executable'],
  [[0xfe, 0xed, 0xfa, 0xce], 'Mach-O executable'],
  [[0xfe, 0xed, 0xfa, 0xcf], 'Mach-O executable'],
  [[0xce, 0xfa, 0xed, 0xfe], 'Mach-O executable'],
  [[0xcf, 0xfa, 0xed, 0xfe], 'Mach-O executable'],
  [[0xca, 0xfe, 0xba, 0xbe], 'Mach-O/Java class'],
  [[0x50, 0x4b, 0x03, 0x04], 'ZIP archive'],
  [[0x1f, 0x8b], 'gzip archive'],
  [[0x52, 0x61, 0x72, 0x21, 0x1a, 0x07], 'RAR archive'],
  [[0x37, 0x7a, 0xbc, 0xaf, 0x27, 0x1c], '7-Zip archive'],
  [[0xa1, 0xb2, 0xc3, 0xd4], 'pcap capture'],
  [[0xd4, 0xc3, 0xb2, 0xa1], 'pcap capture'],
  [[0xa1, 0xb2, 0x3c, 0x4d], 'pcap capture'],
  [[0x4d, 0x3c, 0xb2, 0xa1], 'pcap capture'],
  [[0x0a, 0x0d, 0x0d, 0x0a], 'pcapng capture'],
];

export function refusedSignature(bytes: Uint8Array): string | null {
  return REFUSED_SIGNATURES.find(([sig]) => startsWith(bytes, sig))?.[1] ?? null;
}

/** g9-evidence `MediaType::sniff`: the binary types that carry a signature. */
function sniff(bytes: Uint8Array): AttachmentMediaType | null {
  if (startsWith(bytes, [0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a])) {
    return 'image/png';
  }
  if (startsWith(bytes, [0xff, 0xd8, 0xff])) {
    return 'image/jpeg';
  }
  if (startsWith(bytes, [0x25, 0x50, 0x44, 0x46, 0x2d])) {
    return 'application/pdf';
  }
  return null;
}

/** Strict UTF-8 (a leading BOM is kept as U+FEFF, as Rust's `from_utf8` keeps it). */
function utf8(bytes: Uint8Array): string | null {
  try {
    return new TextDecoder('utf-8', { fatal: true, ignoreBOM: true }).decode(bytes);
  } catch {
    return null;
  }
}

/**
 * g9-evidence `MediaType::consistent_with`: binary types must carry their signature; text types
 * must be UTF-8 without NUL; JSON must parse (serde_json; a leading BOM does not parse).
 */
export function consistentWith(mediaType: AttachmentMediaType, bytes: Uint8Array): boolean {
  switch (mediaType) {
    case 'image/png':
    case 'image/jpeg':
    case 'application/pdf':
      return sniff(bytes) === mediaType;
    case 'text/markdown':
    case 'text/plain':
    case 'text/csv': {
      const text = utf8(bytes);
      return text !== null && !text.includes('\u0000');
    }
    case 'application/json': {
      const text = utf8(bytes);
      if (text === null) {
        return false;
      }
      try {
        JSON.parse(text);
        return true;
      } catch {
        return false;
      }
    }
  }
}
