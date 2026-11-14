import { describe, expect, it } from 'vitest';
import { ERROR_CATEGORIES } from '../ipc/types';
import { canonicalJson } from './canonical-json';
import { civilToUtc, formatLocalDateTime, offsetAt, parseIsoDate, parseLocalDateTime, toZoned } from './civil-time';
import { ERROR_PRESENTATION, allCategoriesPresented } from './error-text';
import { formatCivilDate, formatDuration } from './format';
import { sha256Hex } from './sha256';
import { APPROVAL_MACHINE, ASSESSMENT_MACHINE, BACKUP_MACHINE, SESSION_MACHINE, can, legalEvents, transition } from './state-machines';

describe('canonical JSON and SHA-256', () => {
  it('sorts keys at every level, drops undefined and rejects non-finite numbers', () => {
    expect(canonicalJson({ b: 1, a: { d: [3, { z: 1, y: 2 }], c: null }, u: undefined })).toBe('{"a":{"c":null,"d":[3,{"y":2,"z":1}]},"b":1}');
    expect(() => canonicalJson({ n: Number.NaN })).toThrow(TypeError);
  });

  it('matches FIPS 180-4 known-answer vectors', () => {
    expect(sha256Hex('')).toBe('e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855');
    expect(sha256Hex('abc')).toBe('ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad');
    expect(sha256Hex('abcdbcdecdefdefgefghfghighijhijkijkljklmklmnlmnomnopnopq')).toBe('248d6a61d20638b8e5c026930c3e6039a33ce45964ff2167f6ecedd419db06c1');
  });
});

describe('state-machine tables mirror crates/g9-domain/src/state.rs', () => {
  it('has the same edge counts and terminal states as the Rust tables', () => {
    expect(SESSION_MACHINE.transitions).toHaveLength(15);
    expect(ASSESSMENT_MACHINE.transitions).toHaveLength(10);
    expect(BACKUP_MACHINE.transitions).toHaveLength(9);
    expect(APPROVAL_MACHINE.transitions).toHaveLength(6);
    expect(SESSION_MACHINE.terminal).toEqual(['CLOSED', 'CANCELLED']);
    expect(ASSESSMENT_MACHINE.terminal).toEqual(['APPROVED_FINAL', 'VOIDED']);
  });

  it('finalized attempts and consumed approvals have no outgoing edges; RUNNING is only reachable from READY/PAUSED', () => {
    expect(legalEvents(ASSESSMENT_MACHINE, 'APPROVED_FINAL')).toEqual([]);
    expect(legalEvents(APPROVAL_MACHINE, 'CONSUMED')).toEqual([]);
    expect(can(ASSESSMENT_MACHINE, 'STARTED', 'APPROVE_FINAL')).toBe(false);
    expect(can(BACKUP_MACHINE, 'CHECKED', 'RETAIN')).toBe(false);
    const intoRunning = SESSION_MACHINE.transitions.filter(([, , to]) => to === 'RUNNING').map(([from]) => from);
    expect(intoRunning).toEqual(['READY', 'PAUSED']);
    const r = transition(SESSION_MACHINE, 'INTERRUPTED', 'RESUME');
    expect(r.ok).toBe(false);
  });
});

describe('civil time', () => {
  it('resolves Europe/Berlin winter time and UTC exactly', () => {
    expect(civilToUtc({ year: 2026, month: 11, day: 18, hour: 9, minute: 0 }, 'Europe/Berlin')).toEqual({ utc: Date.UTC(2026, 10, 18, 8, 0), adjusted: null });
    expect(civilToUtc({ year: 2026, month: 11, day: 18, hour: 9, minute: 0 }, 'UTC')).toEqual({ utc: Date.UTC(2026, 10, 18, 9, 0), adjusted: null });
    expect(offsetAt(Date.UTC(2026, 6, 1), 'Europe/Berlin')).toBe(2 * 3_600_000);
    expect(toZoned(Date.UTC(2026, 10, 18, 8, 30), 'Europe/Berlin')).toMatchObject({ hour: 9, minute: 30, weekday: 3 });
  });

  it('reads and writes datetime-local values without a zone of their own', () => {
    expect(parseLocalDateTime('2026-11-19T10:00')).toEqual({ year: 2026, month: 11, day: 19, hour: 10, minute: 0 });
    expect(parseLocalDateTime('2026-11-19T10:00:30')).toEqual({ year: 2026, month: 11, day: 19, hour: 10, minute: 0 });
    for (const bad of ['', '2026-11-19', '2026-02-30T10:00', '2026-11-19T24:00', '2026-11-19 10:00', '19.11.2026 10:00']) {
      expect(parseLocalDateTime(bad), bad).toBeNull();
    }
    expect(formatLocalDateTime(Date.UTC(2026, 10, 19, 9, 0), 'Europe/Berlin')).toBe('2026-11-19T10:00');
    expect(formatLocalDateTime(Date.UTC(2026, 10, 19, 9, 5), 'UTC')).toBe('2026-11-19T09:05');
  });

  it('flags the fall-back overlap and resolves to the earlier (pre-transition) instant; flags the spring gap', () => {
    // 2026-10-25 02:30 happens twice in Berlin; CEST (UTC+2) comes first.
    expect(civilToUtc({ year: 2026, month: 10, day: 25, hour: 2, minute: 30 }, 'Europe/Berlin')).toEqual({ utc: Date.UTC(2026, 9, 25, 0, 30), adjusted: 'OVERLAP' });
    // 2026-03-29 02:30 does not exist in Berlin.
    const gap = civilToUtc({ year: 2026, month: 3, day: 29, hour: 2, minute: 30 }, 'Europe/Berlin');
    expect(gap.adjusted).toBe('GAP');
    expect(toZoned(gap.utc, 'Europe/Berlin').hour).toBe(3);
  });
});

describe('error text and formatting', () => {
  it('maps all 13 error categories to operator text', () => {
    expect(ERROR_CATEGORIES).toHaveLength(13);
    expect(Object.keys(ERROR_PRESENTATION).sort()).toEqual([...ERROR_CATEGORIES].sort());
    expect(allCategoriesPresented()).toBe(true);
  });

  it('formats durations from integer milliseconds', () => {
    expect(formatDuration(0)).toBe('0:00:00');
    expect(formatDuration(15 * 60_000)).toBe('0:15:00');
    expect(formatDuration(3_725_000)).toBe('1:02:05');
  });
});

describe('parseIsoDate', () => {
  it('accepts the days a month has and refuses the days it does not (as the backend does)', () => {
    expect(parseIsoDate('2026-12-31')).toEqual({ year: 2026, month: 12, day: 31 });
    expect(parseIsoDate('2028-02-29')).toEqual({ year: 2028, month: 2, day: 29 });
    for (const bad of ['2026-02-29', '2026-02-30', '2026-04-31', '2026-13-01', '2026-00-10', '2026-01-00', '2026-1-01', '26-01-01', '2026-01-01T00:00']) {
      expect(parseIsoDate(bad), bad).toBeNull();
    }
    // 1900 is not a leap year, 2000 is.
    expect(parseIsoDate('1900-02-29')).toBeNull();
    expect(parseIsoDate('2000-02-29')).not.toBeNull();
  });
});

describe('formatCivilDate', () => {
  it('names the weekday of a civil date and leaves anything else alone', () => {
    expect(formatCivilDate('2026-11-02')).toBe('Monday 2026-11-02');
    expect(formatCivilDate('2026-11-08')).toBe('Sunday 2026-11-08');
    expect(formatCivilDate('2028-02-29')).toBe('Tuesday 2028-02-29');
    expect(formatCivilDate('not a date')).toBe('not a date');
  });
});
