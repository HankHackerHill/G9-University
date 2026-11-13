import { describe, expect, it } from 'vitest';
import { decodeBase64Strict, encodeBase64 } from '../ipc/transports/mock-attachments';
import { ATTACHMENT_MEDIA_TYPES, attachmentMediaType, base64FromDataUrl, formatBytes } from './attachment';
import { sha256Hex, sha256HexBytes } from './sha256';

describe('attachment media types (the evidence.attach_file allow-list)', () => {
  it('keeps an allowed declared type, normalized', () => {
    expect(attachmentMediaType('notes.md', 'text/markdown')).toBe('text/markdown');
    expect(attachmentMediaType('scan', 'Application/PDF; version=1.7')).toBe('application/pdf');
    expect(ATTACHMENT_MEDIA_TYPES).toHaveLength(7);
  });

  it('falls back to the extension when the type is missing or not allowed, and refuses anything else', () => {
    expect(attachmentMediaType('notes.md', '')).toBe('text/markdown');
    expect(attachmentMediaType('notes.MD', 'text/x-markdown')).toBe('text/markdown');
    expect(attachmentMediaType('log.txt', 'application/octet-stream')).toBe('text/plain');
    expect(attachmentMediaType('rows.csv', 'application/vnd.ms-excel')).toBe('text/csv');
    expect(attachmentMediaType('data.json', '')).toBe('application/json');
    expect(attachmentMediaType('shot.JPG', 'image/jpg')).toBe('image/jpeg');
    expect(attachmentMediaType('shot.jpeg', '')).toBe('image/jpeg');
    expect(attachmentMediaType('image.png', '')).toBe('image/png');
    expect(attachmentMediaType('report.pdf', '')).toBe('application/pdf');
    expect(attachmentMediaType('tool.exe', 'application/x-msdownload')).toBeNull();
    expect(attachmentMediaType('archive.zip', 'application/zip')).toBeNull();
    expect(attachmentMediaType('README', '')).toBeNull();
    expect(attachmentMediaType('.md', '')).toBeNull();
    expect(attachmentMediaType('trailing.', '')).toBeNull();
  });

  it('extracts the base64 of a data URL without its prefix', () => {
    expect(base64FromDataUrl('data:text/plain;base64,aGk=')).toBe('aGk=');
    expect(base64FromDataUrl('data:application/octet-stream;base64,')).toBe('');
    expect(base64FromDataUrl('data:text/plain,hi')).toBeNull();
    expect(base64FromDataUrl('aGk=')).toBeNull();
  });

  it('formats sizes for people', () => {
    expect(formatBytes(512)).toBe('512 B');
    expect(formatBytes(1536)).toBe('1.5 KiB');
    expect(formatBytes(16 * 1024 * 1024)).toBe('16 MiB');
  });
});

describe('strict base64 and byte hashing (mock transport)', () => {
  it('round-trips bytes and refuses what the base64 crate STANDARD engine refuses', () => {
    const bytes = new Uint8Array([0, 1, 2, 250, 251, 252, 253, 254, 255, 104, 105]);
    for (let n = 0; n <= bytes.length; n += 1) {
      const slice = bytes.slice(0, n);
      expect(decodeBase64Strict(encodeBase64(slice))).toEqual(slice);
    }
    expect(encodeBase64(new TextEncoder().encode('hi'))).toBe('aGk=');
    for (const bad of ['aGk', 'aGl=', 'a$k=', '%%%%', '====', 'a===', 'aGk=aGk=', 'aG k=', 'aGk=\n']) {
      expect(decodeBase64Strict(bad), bad).toBeNull();
    }
  });

  it('hashes bytes with the FIPS 180-4 known answers, consistently with the text form', () => {
    expect(sha256HexBytes(new Uint8Array())).toBe('e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855');
    expect(sha256HexBytes(new Uint8Array([0x61, 0x62, 0x63]))).toBe('ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad');
    const text = 'X-Forwarded-Host is unkeyed.\n';
    expect(sha256HexBytes(new TextEncoder().encode(text))).toBe(sha256Hex(text));
    // Bytes that are not UTF-8 hash as themselves.
    expect(sha256HexBytes(new Uint8Array([0xff]))).toBe('a8100ae6aa1940d0b663bb31cd466142ebbdbd5187131b92d93818987832eb89');
  });
});
