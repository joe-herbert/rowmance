import { describe, it, expect } from 'vitest';
import { resolveBuiltinVariables, substituteVariables } from './widget-templates';

describe('resolveBuiltinVariables', () => {
  it('falls back to the epoch when the dashboard has never been viewed', () => {
    const vars = resolveBuiltinVariables({
      dashboardId: 'd1',
      widgetId: 'w1',
      lastViewedAt: null,
    });
    expect(vars.DASHBOARD_LAST_VIEWED_AT_UTC).toBe(new Date(0).toISOString());
  });

  it('uses the provided last-viewed timestamp for the UTC variant', () => {
    const vars = resolveBuiltinVariables({
      dashboardId: 'd1',
      widgetId: 'w1',
      lastViewedAt: '2026-01-01T00:00:00.000Z',
    });
    expect(vars.DASHBOARD_LAST_VIEWED_AT_UTC).toBe('2026-01-01T00:00:00.000Z');
  });

  it('includes dashboard and widget ids', () => {
    const vars = resolveBuiltinVariables({ dashboardId: 'd1', widgetId: 'w1', lastViewedAt: null });
    expect(vars.DASHBOARD_ID).toBe('d1');
    expect(vars.WIDGET_ID).toBe('w1');
  });

  it('formats the local (bare-name) variants without a UTC offset, using local wall-clock components', () => {
    const iso = '2026-01-01T00:00:00.000Z';
    const vars = resolveBuiltinVariables({ dashboardId: 'd1', widgetId: 'w1', lastViewedAt: iso });
    const expected = new Date(iso);
    const pad = (n: number) => String(n).padStart(2, '0');
    const wantLocal =
      `${expected.getFullYear()}-${pad(expected.getMonth() + 1)}-${pad(expected.getDate())}` +
      `T${pad(expected.getHours())}:${pad(expected.getMinutes())}:${pad(expected.getSeconds())}`;
    expect(vars.DASHBOARD_LAST_VIEWED_AT).toBe(wantLocal);
    expect(vars.NOW).not.toContain('Z');
    expect(vars.TODAY).toMatch(/^\d{4}-\d{2}-\d{2}$/);
  });

  describe('day/week/month/year boundaries', () => {
    const vars = resolveBuiltinVariables({ dashboardId: 'd1', widgetId: 'w1', lastViewedAt: null });

    it('pins START/END_OF_DAY_UTC to midnight and the last instant of the UTC day', () => {
      expect(vars.START_OF_DAY_UTC).toMatch(/T00:00:00\.000Z$/);
      expect(vars.END_OF_DAY_UTC).toMatch(/T23:59:59\.999Z$/);
    });

    it('keeps START_OF_* before or equal to END_OF_* for every period', () => {
      for (const period of ['DAY', 'WEEK', 'MONTH', 'YEAR']) {
        for (const suffix of ['', '_UTC']) {
          const start = new Date(vars[`START_OF_${period}${suffix}`]);
          const end = new Date(vars[`END_OF_${period}${suffix}`]);
          expect(start.getTime()).toBeLessThanOrEqual(end.getTime());
        }
      }
    });

    it('anchors START_OF_WEEK_UTC on a Monday', () => {
      expect(new Date(vars.START_OF_WEEK_UTC).getUTCDay()).toBe(1);
    });

    it('anchors START_OF_MONTH_UTC on the 1st and END_OF_MONTH_UTC on the last day of the month', () => {
      const start = new Date(vars.START_OF_MONTH_UTC);
      const end = new Date(vars.END_OF_MONTH_UTC);
      expect(start.getUTCDate()).toBe(1);
      expect(end.getUTCMonth()).toBe(start.getUTCMonth());
    });

    it('anchors START_OF_YEAR_UTC on Jan 1st and END_OF_YEAR_UTC on Dec 31st', () => {
      const start = new Date(vars.START_OF_YEAR_UTC);
      const end = new Date(vars.END_OF_YEAR_UTC);
      expect(start.getUTCMonth()).toBe(0);
      expect(start.getUTCDate()).toBe(1);
      expect(end.getUTCMonth()).toBe(11);
      expect(end.getUTCDate()).toBe(31);
    });

    it('formats the local variants without a UTC offset', () => {
      for (const period of ['DAY', 'WEEK', 'MONTH', 'YEAR']) {
        expect(vars[`START_OF_${period}`]).not.toContain('Z');
        expect(vars[`END_OF_${period}`]).not.toContain('Z');
      }
    });
  });
});

describe('substituteVariables', () => {
  it('replaces a built-in token', () => {
    const sql = substituteVariables(
      "SELECT * FROM events WHERE created_at > '{{DASHBOARD_LAST_VIEWED_AT_UTC}}'",
      { DASHBOARD_LAST_VIEWED_AT_UTC: '2026-01-01T00:00:00.000Z' },
    );
    expect(sql).toBe("SELECT * FROM events WHERE created_at > '2026-01-01T00:00:00.000Z'");
  });

  it('replaces a custom variable', () => {
    const sql = substituteVariables('SELECT * FROM t WHERE status = {{STATUS}}', {}, [
      { name: 'STATUS', value: "'active'" },
    ]);
    expect(sql).toBe("SELECT * FROM t WHERE status = 'active'");
  });

  it('lets custom variables override built-ins of the same name', () => {
    const sql = substituteVariables('{{NOW}}', { NOW: 'builtin' }, [
      { name: 'NOW', value: 'overridden' },
    ]);
    expect(sql).toBe('overridden');
  });

  it('leaves unknown tokens untouched', () => {
    const sql = substituteVariables('{{UNKNOWN}}', {});
    expect(sql).toBe('{{UNKNOWN}}');
  });

  it('replaces multiple occurrences of the same variable', () => {
    const sql = substituteVariables('{{X}} = {{X}}', { X: '1' });
    expect(sql).toBe('1 = 1');
  });
});
