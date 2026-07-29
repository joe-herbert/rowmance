import type { DashboardVariable } from '$lib/types';

export interface BuiltinVariableInfo {
  name: string;
  description: string;
  category: string;
}

/** Built-in `{{NAME}}` tokens available in every widget query, in addition to custom variables. */
export const BUILTIN_VARIABLES: BuiltinVariableInfo[] = [
  {
    name: 'DASHBOARD_LAST_VIEWED_AT',
    description:
      'Timestamp of when this dashboard was last viewed, in your local timezone (epoch if never viewed)',
    category: 'Last viewed',
  },
  {
    name: 'DASHBOARD_LAST_VIEWED_AT_UTC',
    description: 'Same as DASHBOARD_LAST_VIEWED_AT, in UTC — use if your column stores UTC',
    category: 'Last viewed',
  },
  { name: 'NOW', description: 'Current timestamp in your local timezone', category: 'Now & today' },
  { name: 'NOW_UTC', description: 'Current UTC timestamp', category: 'Now & today' },
  {
    name: 'TODAY',
    description: 'Current date in your local timezone (YYYY-MM-DD)',
    category: 'Now & today',
  },
  { name: 'TODAY_UTC', description: 'Current UTC date (YYYY-MM-DD)', category: 'Now & today' },
  {
    name: 'START_OF_DAY',
    description: 'Midnight today, in your local timezone',
    category: 'Day',
  },
  { name: 'START_OF_DAY_UTC', description: 'Midnight today, in UTC', category: 'Day' },
  {
    name: 'END_OF_DAY',
    description: 'The last instant of today (23:59:59.999), in your local timezone',
    category: 'Day',
  },
  {
    name: 'END_OF_DAY_UTC',
    description: 'The last instant of today (23:59:59.999), in UTC',
    category: 'Day',
  },
  {
    name: 'START_OF_WEEK',
    description: 'Midnight on Monday of this week, in your local timezone',
    category: 'Week',
  },
  {
    name: 'START_OF_WEEK_UTC',
    description: 'Midnight on Monday of this week, in UTC',
    category: 'Week',
  },
  {
    name: 'END_OF_WEEK',
    description: 'The last instant of Sunday this week, in your local timezone',
    category: 'Week',
  },
  {
    name: 'END_OF_WEEK_UTC',
    description: 'The last instant of Sunday this week, in UTC',
    category: 'Week',
  },
  {
    name: 'START_OF_MONTH',
    description: 'Midnight on the 1st of this month, in your local timezone',
    category: 'Month',
  },
  {
    name: 'START_OF_MONTH_UTC',
    description: 'Midnight on the 1st of this month, in UTC',
    category: 'Month',
  },
  {
    name: 'END_OF_MONTH',
    description: 'The last instant of this month, in your local timezone',
    category: 'Month',
  },
  {
    name: 'END_OF_MONTH_UTC',
    description: 'The last instant of this month, in UTC',
    category: 'Month',
  },
  {
    name: 'START_OF_YEAR',
    description: 'Midnight on Jan 1st of this year, in your local timezone',
    category: 'Year',
  },
  {
    name: 'START_OF_YEAR_UTC',
    description: 'Midnight on Jan 1st of this year, in UTC',
    category: 'Year',
  },
  {
    name: 'END_OF_YEAR',
    description: 'The last instant of this year, in your local timezone',
    category: 'Year',
  },
  {
    name: 'END_OF_YEAR_UTC',
    description: 'The last instant of this year, in UTC',
    category: 'Year',
  },
  { name: 'DASHBOARD_ID', description: 'ID of the current dashboard', category: 'Identifiers' },
  { name: 'WIDGET_ID', description: 'ID of the current widget', category: 'Identifiers' },
];

export interface BuiltinVariableContext {
  dashboardId: string;
  widgetId: string;
  /** Previous last-viewed timestamp for the dashboard, or null if never viewed before. */
  lastViewedAt: string | null;
}

/** Formats a Date using local wall-clock components, with no UTC offset/Z suffix. */
function toLocalNaiveIso(date: Date): string {
  const pad = (n: number) => String(n).padStart(2, '0');
  return (
    `${date.getFullYear()}-${pad(date.getMonth() + 1)}-${pad(date.getDate())}` +
    `T${pad(date.getHours())}:${pad(date.getMinutes())}:${pad(date.getSeconds())}`
  );
}

/** Offset in days from `day` (0=Sun..6=Sat) back to that week's Monday. */
function mondayOffset(day: number): number {
  return day === 0 ? -6 : 1 - day;
}

// ── Local-timezone day/week/month/year boundaries ──────────────────────────────

function startOfDayLocal(d: Date): Date {
  return new Date(d.getFullYear(), d.getMonth(), d.getDate());
}
function endOfDayLocal(d: Date): Date {
  return new Date(d.getFullYear(), d.getMonth(), d.getDate(), 23, 59, 59, 999);
}
function startOfWeekLocal(d: Date): Date {
  return new Date(d.getFullYear(), d.getMonth(), d.getDate() + mondayOffset(d.getDay()));
}
function endOfWeekLocal(d: Date): Date {
  const start = startOfWeekLocal(d);
  return new Date(start.getFullYear(), start.getMonth(), start.getDate() + 6, 23, 59, 59, 999);
}
function startOfMonthLocal(d: Date): Date {
  return new Date(d.getFullYear(), d.getMonth(), 1);
}
function endOfMonthLocal(d: Date): Date {
  return new Date(d.getFullYear(), d.getMonth() + 1, 0, 23, 59, 59, 999);
}
function startOfYearLocal(d: Date): Date {
  return new Date(d.getFullYear(), 0, 1);
}
function endOfYearLocal(d: Date): Date {
  return new Date(d.getFullYear(), 11, 31, 23, 59, 59, 999);
}

// ── UTC day/week/month/year boundaries ─────────────────────────────────────────

function startOfDayUtc(d: Date): Date {
  return new Date(Date.UTC(d.getUTCFullYear(), d.getUTCMonth(), d.getUTCDate()));
}
function endOfDayUtc(d: Date): Date {
  return new Date(Date.UTC(d.getUTCFullYear(), d.getUTCMonth(), d.getUTCDate(), 23, 59, 59, 999));
}
function startOfWeekUtc(d: Date): Date {
  return new Date(
    Date.UTC(d.getUTCFullYear(), d.getUTCMonth(), d.getUTCDate() + mondayOffset(d.getUTCDay())),
  );
}
function endOfWeekUtc(d: Date): Date {
  const start = startOfWeekUtc(d);
  return new Date(
    Date.UTC(start.getUTCFullYear(), start.getUTCMonth(), start.getUTCDate() + 6, 23, 59, 59, 999),
  );
}
function startOfMonthUtc(d: Date): Date {
  return new Date(Date.UTC(d.getUTCFullYear(), d.getUTCMonth(), 1));
}
function endOfMonthUtc(d: Date): Date {
  return new Date(Date.UTC(d.getUTCFullYear(), d.getUTCMonth() + 1, 0, 23, 59, 59, 999));
}
function startOfYearUtc(d: Date): Date {
  return new Date(Date.UTC(d.getUTCFullYear(), 0, 1));
}
function endOfYearUtc(d: Date): Date {
  return new Date(Date.UTC(d.getUTCFullYear(), 11, 31, 23, 59, 59, 999));
}

export function resolveBuiltinVariables(ctx: BuiltinVariableContext): Record<string, string> {
  const now = new Date();
  const lastViewed = ctx.lastViewedAt ? new Date(ctx.lastViewedAt) : new Date(0);
  return {
    DASHBOARD_LAST_VIEWED_AT: toLocalNaiveIso(lastViewed),
    DASHBOARD_LAST_VIEWED_AT_UTC: lastViewed.toISOString(),
    NOW: toLocalNaiveIso(now),
    NOW_UTC: now.toISOString(),
    TODAY: toLocalNaiveIso(now).slice(0, 10),
    TODAY_UTC: now.toISOString().slice(0, 10),
    START_OF_DAY: toLocalNaiveIso(startOfDayLocal(now)),
    START_OF_DAY_UTC: startOfDayUtc(now).toISOString(),
    END_OF_DAY: toLocalNaiveIso(endOfDayLocal(now)),
    END_OF_DAY_UTC: endOfDayUtc(now).toISOString(),
    START_OF_WEEK: toLocalNaiveIso(startOfWeekLocal(now)),
    START_OF_WEEK_UTC: startOfWeekUtc(now).toISOString(),
    END_OF_WEEK: toLocalNaiveIso(endOfWeekLocal(now)),
    END_OF_WEEK_UTC: endOfWeekUtc(now).toISOString(),
    START_OF_MONTH: toLocalNaiveIso(startOfMonthLocal(now)),
    START_OF_MONTH_UTC: startOfMonthUtc(now).toISOString(),
    END_OF_MONTH: toLocalNaiveIso(endOfMonthLocal(now)),
    END_OF_MONTH_UTC: endOfMonthUtc(now).toISOString(),
    START_OF_YEAR: toLocalNaiveIso(startOfYearLocal(now)),
    START_OF_YEAR_UTC: startOfYearUtc(now).toISOString(),
    END_OF_YEAR: toLocalNaiveIso(endOfYearLocal(now)),
    END_OF_YEAR_UTC: endOfYearUtc(now).toISOString(),
    DASHBOARD_ID: ctx.dashboardId,
    WIDGET_ID: ctx.widgetId,
  };
}

/**
 * Replaces `{{NAME}}` tokens in `sql` with values from `variables`, dashboard variables
 * taking precedence over built-ins of the same name. Unknown tokens are left untouched.
 */
export function substituteVariables(
  sql: string,
  builtins: Record<string, string>,
  dashboardVariables: DashboardVariable[] = [],
): string {
  const values: Record<string, string> = { ...builtins };
  for (const v of dashboardVariables) {
    if (v.name.trim()) values[v.name.trim()] = v.value;
  }
  return sql.replace(/\{\{\s*([A-Za-z0-9_]+)\s*\}\}/g, (match, name: string) =>
    name in values ? values[name] : match,
  );
}
