/**
 * What the window lets the operator attach (Learning Journal), before anything is sent.
 * Mirrors the IPC contract of `evidence.attach_file` (schemas/ipc/evidence.attach_file.request,
 * crates/g9-app/src/commands/evidence.rs); the backend re-checks every rule and also checks the
 * bytes themselves (signatures, UTF-8, JSON), which the window does not try to judge.
 */
import type { AttachmentMediaType } from '../ipc/operations';

/** The IPC allow-list (g9-evidence `MediaType::ALLOWED`, the request schema's `media_type` enum). */
export const ATTACHMENT_MEDIA_TYPES: readonly AttachmentMediaType[] = [
  'text/markdown',
  'text/plain',
  'application/pdf',
  'image/png',
  'image/jpeg',
  'application/json',
  'text/csv',
];

/** Largest file attached through the app, in decoded bytes (evidence.rs `MAX_UPLOAD_BYTES`, 16 MiB). */
export const MAX_ATTACHMENT_BYTES = 16 * 1024 * 1024;

/** Extension fallbacks for a file whose declared type is missing or not on the allow-list. */
const BY_EXTENSION: Readonly<Record<string, AttachmentMediaType>> = {
  md: 'text/markdown',
  txt: 'text/plain',
  csv: 'text/csv',
  json: 'application/json',
  pdf: 'application/pdf',
  png: 'image/png',
  jpg: 'image/jpeg',
  jpeg: 'image/jpeg',
};

export function isAttachmentMediaType(value: string): value is AttachmentMediaType {
  return (ATTACHMENT_MEDIA_TYPES as readonly string[]).includes(value);
}

/**
 * The allow-listed media type to declare for a picked file: its own type when that is on the
 * allow-list (parameters dropped, case ignored), otherwise the type its extension names.
 * `null` means the window refuses the file before sending anything.
 */
export function attachmentMediaType(fileName: string, declaredType: string): AttachmentMediaType | null {
  const essence = (declaredType.split(';')[0] ?? '').trim().toLowerCase();
  if (isAttachmentMediaType(essence)) {
    return essence;
  }
  const dot = fileName.lastIndexOf('.');
  if (dot <= 0 || dot === fileName.length - 1) {
    return null;
  }
  return BY_EXTENSION[fileName.slice(dot + 1).toLowerCase()] ?? null;
}

/** The base64 text of a `data:…;base64,` URL (what `FileReader.readAsDataURL` yields), without the prefix; null otherwise. */
export function base64FromDataUrl(url: string): string | null {
  const match = /^data:[^,]*;base64,/.exec(url);
  return match === null ? null : url.slice(match[0].length);
}

/** `1536` → `1.5 KiB`: a size for people; the exact byte count is shown next to it where it matters. */
export function formatBytes(bytes: number): string {
  if (bytes < 1024) {
    return `${bytes} B`;
  }
  const units = ['KiB', 'MiB', 'GiB'];
  let value = bytes / 1024;
  let unit = 0;
  while (value >= 1024 && unit < units.length - 1) {
    value /= 1024;
    unit += 1;
  }
  return `${value.toFixed(value < 10 ? 1 : 0)} ${units[unit] ?? 'GiB'}`;
}
