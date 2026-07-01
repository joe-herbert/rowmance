import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';

// Pure functions redefined locally to avoid rune compilation issues in test context.
// These mirror the inline functions in RightSidebar.svelte.

function formatDuration(ms: number | null): string {
  if (ms === null) return '';
  if (ms < 1000) return `${ms}ms`;
  return `${(ms / 1000).toFixed(1)}s`;
}

function timeAgo(isoString: string): string {
  const diff = Date.now() - new Date(isoString).getTime();
  const mins = Math.floor(diff / 60_000);
  if (mins < 1) return 'just now';
  if (mins < 60) return `${mins}m ago`;
  const hrs = Math.floor(mins / 60);
  if (hrs < 24) return `${hrs}h ago`;
  return `${Math.floor(hrs / 24)}d ago`;
}

function truncateSql(sql: string, max = 80): string {
  const cleaned = sql.replace(/\s+/g, ' ').trim();
  return cleaned.length > max ? cleaned.slice(0, max) + '…' : cleaned;
}

// ── formatDuration tests ──────────────────────────────────────────────────────

describe('formatDuration', () => {
  it('returns empty string for null', () => {
    expect(formatDuration(null)).toBe('');
  });

  it('returns "0ms" for 0', () => {
    expect(formatDuration(0)).toBe('0ms');
  });

  it('returns "999ms" for 999', () => {
    expect(formatDuration(999)).toBe('999ms');
  });

  it('returns "1.0s" for 1000', () => {
    expect(formatDuration(1000)).toBe('1.0s');
  });

  it('returns "1.5s" for 1500', () => {
    expect(formatDuration(1500)).toBe('1.5s');
  });

  it('returns "60.0s" for 60000', () => {
    expect(formatDuration(60000)).toBe('60.0s');
  });
});

// ── timeAgo tests ─────────────────────────────────────────────────────────────

describe('timeAgo', () => {
  const FIXED_NOW = new Date('2024-01-15T12:00:00.000Z').getTime();

  beforeEach(() => {
    vi.useFakeTimers();
    vi.setSystemTime(FIXED_NOW);
  });

  afterEach(() => {
    vi.useRealTimers();
  });

  it('returns "just now" for a date 30 seconds ago', () => {
    const date = new Date(FIXED_NOW - 30_000).toISOString();
    expect(timeAgo(date)).toBe('just now');
  });

  it('returns "just now" for a date 59 seconds ago', () => {
    const date = new Date(FIXED_NOW - 59_000).toISOString();
    expect(timeAgo(date)).toBe('just now');
  });

  it('returns "2m ago" for a date 2 minutes ago', () => {
    const date = new Date(FIXED_NOW - 2 * 60_000).toISOString();
    expect(timeAgo(date)).toBe('2m ago');
  });

  it('returns "3h ago" for a date 3 hours ago', () => {
    const date = new Date(FIXED_NOW - 3 * 60 * 60_000).toISOString();
    expect(timeAgo(date)).toBe('3h ago');
  });

  it('returns "2d ago" for a date 2 days ago', () => {
    const date = new Date(FIXED_NOW - 2 * 24 * 60 * 60_000).toISOString();
    expect(timeAgo(date)).toBe('2d ago');
  });
});

// ── truncateSql tests ─────────────────────────────────────────────────────────

describe('truncateSql', () => {
  it('passes through short SQL unchanged', () => {
    const sql = 'SELECT * FROM users';
    expect(truncateSql(sql)).toBe('SELECT * FROM users');
  });

  it('truncates long SQL and appends ellipsis character', () => {
    const sql =
      'SELECT id, name, email, address, phone, status FROM users WHERE active = 1 ORDER BY name';
    const result = truncateSql(sql);
    expect(result.length).toBe(81); // 80 chars + '…'
    expect(result.endsWith('…')).toBe(true);
  });

  it('collapses multiple whitespace to single space', () => {
    const sql = 'SELECT  *   FROM   users';
    expect(truncateSql(sql)).toBe('SELECT * FROM users');
  });

  it('trims leading and trailing whitespace', () => {
    const sql = '  SELECT * FROM users  ';
    expect(truncateSql(sql)).toBe('SELECT * FROM users');
  });

  it('collapses newlines and tabs to single space', () => {
    const sql = 'SELECT\n  *\n  FROM\n\tusers';
    expect(truncateSql(sql)).toBe('SELECT * FROM users');
  });

  it('respects custom max length', () => {
    const sql = 'SELECT * FROM users WHERE id = 1';
    const result = truncateSql(sql, 10);
    expect(result).toBe('SELECT * F…');
    expect(result.length).toBe(11); // 10 chars + '…'
  });

  it('does not truncate SQL exactly at max length', () => {
    const sql = 'A'.repeat(80);
    const result = truncateSql(sql);
    expect(result).toBe(sql); // exactly 80 — no truncation
    expect(result.endsWith('…')).toBe(false);
  });

  it('truncates SQL that is one character over max', () => {
    const sql = 'A'.repeat(81);
    const result = truncateSql(sql);
    expect(result.endsWith('…')).toBe(true);
    expect(result.length).toBe(81); // 80 + '…'
  });
});
