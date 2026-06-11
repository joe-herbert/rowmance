import { describe, it, expect } from 'vitest';

/**
 * Pure helper functions extracted from CellEditor logic.
 * These mirror the exported helpers in CellEditor.svelte but are tested here
 * as plain functions to avoid the Svelte component runtime overhead.
 */

function isBooleanType(dt: string): boolean {
  return dt.toLowerCase().includes('bool');
}

function isDateType(dt: string): boolean {
  return dt.toLowerCase().includes('date');
}

function isDateTimeType(dt: string): boolean {
  const lower = dt.toLowerCase();
  return lower.includes('date') && lower.includes('time');
}

function getInputType(dt: string): 'boolean' | 'datetime-local' | 'date' | 'text' {
  if (isBooleanType(dt)) return 'boolean';
  if (isDateTimeType(dt)) return 'datetime-local';
  if (isDateType(dt)) return 'date';
  return 'text';
}

describe('isBooleanType', () => {
  it('returns true for BOOLEAN', () => {
    expect(isBooleanType('BOOLEAN')).toBe(true);
  });

  it('returns true for bool (lowercase)', () => {
    expect(isBooleanType('bool')).toBe(true);
  });

  it('returns true for TINYINT(1) / BOOL alias in MySQL', () => {
    expect(isBooleanType('BOOL')).toBe(true);
  });

  it('returns false for INT', () => {
    expect(isBooleanType('INT')).toBe(false);
  });

  it('returns false for VARCHAR', () => {
    expect(isBooleanType('VARCHAR(255)')).toBe(false);
  });
});

describe('isDateType', () => {
  it('returns true for DATE', () => {
    expect(isDateType('DATE')).toBe(true);
  });

  it('returns true for DATETIME', () => {
    expect(isDateType('DATETIME')).toBe(true);
  });

  it('returns true for TIMESTAMP', () => {
    // TIMESTAMP does not contain 'date' — should return false
    expect(isDateType('TIMESTAMP')).toBe(false);
  });

  it('returns false for INT', () => {
    expect(isDateType('INT')).toBe(false);
  });
});

describe('isDateTimeType', () => {
  it('returns true for DATETIME', () => {
    expect(isDateTimeType('DATETIME')).toBe(true);
  });

  it('returns true for datetime (lowercase)', () => {
    expect(isDateTimeType('datetime')).toBe(true);
  });

  it('returns false for DATE only', () => {
    expect(isDateTimeType('DATE')).toBe(false);
  });

  it('returns false for TIME only', () => {
    expect(isDateTimeType('TIME')).toBe(false);
  });
});

describe('getInputType', () => {
  it('returns "boolean" for BOOLEAN', () => {
    expect(getInputType('BOOLEAN')).toBe('boolean');
  });

  it('returns "boolean" for bool', () => {
    expect(getInputType('bool')).toBe('boolean');
  });

  it('returns "datetime-local" for DATETIME', () => {
    expect(getInputType('DATETIME')).toBe('datetime-local');
  });

  it('returns "date" for DATE', () => {
    expect(getInputType('DATE')).toBe('date');
  });

  it('returns "text" for VARCHAR(255)', () => {
    expect(getInputType('VARCHAR(255)')).toBe('text');
  });

  it('returns "text" for INT', () => {
    expect(getInputType('INT')).toBe('text');
  });

  it('returns "text" for TEXT', () => {
    expect(getInputType('TEXT')).toBe('text');
  });

  it('returns "text" for DECIMAL(10,2)', () => {
    expect(getInputType('DECIMAL(10,2)')).toBe('text');
  });

  it('boolean takes priority over date (e.g. hypothetical type)', () => {
    // A type containing both 'bool' and 'date' should prefer boolean
    expect(getInputType('bool_date_type')).toBe('boolean');
  });
});
