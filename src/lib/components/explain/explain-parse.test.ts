import { describe, it, expect } from 'vitest';

// Mirror of the parsing functions from ExplainCanvas.svelte so they can be tested in isolation.

interface ExplainNode {
  id: string;
  nodeType: string;
  relation: string | null;
  estimatedRows: number;
  actualRows: number | null;
  cost: number;
  children: ExplainNode[];
}

let counter = 0;
function nextId() {
  return `n${counter++}`;
}

function resetCounter() {
  counter = 0;
}

function buildNodeFromPgPlan(plan: Record<string, unknown>): ExplainNode {
  const children = ((plan['Plans'] as unknown[]) ?? []).map((p) =>
    buildNodeFromPgPlan(p as Record<string, unknown>),
  );
  return {
    id: nextId(),
    nodeType: String(plan['Node Type'] ?? 'Unknown'),
    relation: plan['Relation Name'] ? String(plan['Relation Name']) : null,
    estimatedRows: Number(plan['Plan Rows'] ?? 0),
    actualRows: plan['Actual Rows'] != null ? Number(plan['Actual Rows']) : null,
    cost: Number(plan['Total Cost'] ?? 0),
    children,
  };
}

function parsePostgres(raw: string): ExplainNode | null {
  try {
    const arr = JSON.parse(raw);
    const plan = Array.isArray(arr) ? arr[0]?.Plan : arr?.Plan;
    if (!plan) return null;
    return buildNodeFromPgPlan(plan);
  } catch {
    return null;
  }
}

function buildNodeFromMysqlTable(table: Record<string, unknown>): ExplainNode {
  const costInfo = table['cost_info'] as Record<string, unknown> | undefined;
  return {
    id: nextId(),
    nodeType: String(table['access_type'] ?? 'table'),
    relation: table['table_name'] ? String(table['table_name']) : null,
    estimatedRows: Number(table['rows_examined_per_scan'] ?? table['rows_produced_per_join'] ?? 0),
    actualRows: null,
    cost: Number(costInfo?.['read_cost'] ?? costInfo?.['eval_cost'] ?? 0),
    children: [],
  };
}

function buildNodeFromMysqlBlock(block: Record<string, unknown>): ExplainNode {
  const children: ExplainNode[] = [];
  const nested = block['nested_loop'] as Record<string, unknown>[] | undefined;
  if (nested) {
    for (const item of nested) {
      const table = item['table'] as Record<string, unknown> | undefined;
      if (table) children.push(buildNodeFromMysqlTable(table));
    }
  }
  const tableNode = block['table'] as Record<string, unknown> | undefined;
  if (tableNode) children.push(buildNodeFromMysqlTable(tableNode));
  return {
    id: nextId(),
    nodeType: 'Query Block',
    relation: null,
    estimatedRows: Number(block['select_id'] ?? 0),
    actualRows: null,
    cost: 0,
    children,
  };
}

function parseMysql(raw: string): ExplainNode | null {
  try {
    const obj = JSON.parse(raw);
    const queryBlock = obj?.query_block ?? obj;
    return buildNodeFromMysqlBlock(queryBlock);
  } catch {
    return null;
  }
}

// Cost-threshold logic (top-20% of costs)
function computeHighCostThreshold(costs: number[]): number {
  if (costs.length === 0) return 0;
  const sorted = [...costs].sort((a, b) => b - a);
  const idx = Math.max(0, Math.floor(sorted.length * 0.2) - 1);
  return sorted[idx] ?? 0;
}

// ── Tests ─────────────────────────────────────────────────────────────────────

describe('parsePostgres', () => {
  beforeEach(() => resetCounter());

  it('returns null for invalid JSON', () => {
    expect(parsePostgres('not json')).toBeNull();
  });

  it('returns null when Plan key is missing', () => {
    expect(parsePostgres('[{"NoPlan": {}}]')).toBeNull();
  });

  it('parses a simple Seq Scan plan', () => {
    const raw = JSON.stringify([{
      Plan: {
        'Node Type': 'Seq Scan',
        'Relation Name': 'users',
        'Plan Rows': 100,
        'Total Cost': 5.5,
        'Actual Rows': 98,
      },
    }]);
    const root = parsePostgres(raw);
    expect(root).not.toBeNull();
    expect(root!.nodeType).toBe('Seq Scan');
    expect(root!.relation).toBe('users');
    expect(root!.estimatedRows).toBe(100);
    expect(root!.actualRows).toBe(98);
    expect(root!.cost).toBe(5.5);
    expect(root!.children).toHaveLength(0);
  });

  it('parses nested Plans into children', () => {
    const raw = JSON.stringify([{
      Plan: {
        'Node Type': 'Hash Join',
        'Plan Rows': 200,
        'Total Cost': 30.0,
        Plans: [
          { 'Node Type': 'Seq Scan', 'Relation Name': 'a', 'Plan Rows': 10, 'Total Cost': 5.0 },
          { 'Node Type': 'Hash', 'Plan Rows': 10, 'Total Cost': 5.0 },
        ],
      },
    }]);
    const root = parsePostgres(raw);
    expect(root!.nodeType).toBe('Hash Join');
    expect(root!.children).toHaveLength(2);
    expect(root!.children[0].nodeType).toBe('Seq Scan');
    expect(root!.children[1].nodeType).toBe('Hash');
  });

  it('handles object format (non-array) with Plan key', () => {
    const raw = JSON.stringify({
      Plan: { 'Node Type': 'Index Scan', 'Plan Rows': 1, 'Total Cost': 1.0 },
    });
    const root = parsePostgres(raw);
    expect(root).not.toBeNull();
    expect(root!.nodeType).toBe('Index Scan');
  });

  it('defaults actualRows to null when Actual Rows is absent', () => {
    const raw = JSON.stringify([{
      Plan: { 'Node Type': 'Seq Scan', 'Plan Rows': 10, 'Total Cost': 2.0 },
    }]);
    const root = parsePostgres(raw);
    expect(root!.actualRows).toBeNull();
  });
});

describe('parseMysql', () => {
  beforeEach(() => resetCounter());

  it('returns null for invalid JSON', () => {
    expect(parseMysql('{')).toBeNull();
  });

  it('parses a simple query block with a table', () => {
    const raw = JSON.stringify({
      query_block: {
        select_id: 1,
        table: {
          table_name: 'users',
          access_type: 'ALL',
          rows_examined_per_scan: 500,
          cost_info: { read_cost: '10.5', eval_cost: '2.0' },
        },
      },
    });
    const root = parseMysql(raw);
    expect(root).not.toBeNull();
    expect(root!.nodeType).toBe('Query Block');
    expect(root!.children).toHaveLength(1);
    expect(root!.children[0].relation).toBe('users');
    expect(root!.children[0].nodeType).toBe('ALL');
    expect(root!.children[0].cost).toBe(10.5);
  });

  it('parses nested_loop into multiple children', () => {
    const raw = JSON.stringify({
      query_block: {
        select_id: 1,
        nested_loop: [
          { table: { table_name: 'orders', access_type: 'index', rows_examined_per_scan: 10 } },
          { table: { table_name: 'users', access_type: 'eq_ref', rows_examined_per_scan: 1 } },
        ],
      },
    });
    const root = parseMysql(raw);
    expect(root!.children).toHaveLength(2);
    expect(root!.children[0].relation).toBe('orders');
    expect(root!.children[1].relation).toBe('users');
  });

  it('uses top-level object as query block when query_block key is absent', () => {
    const raw = JSON.stringify({
      select_id: 1,
      table: { table_name: 't', access_type: 'ALL', rows_examined_per_scan: 5 },
    });
    const root = parseMysql(raw);
    expect(root).not.toBeNull();
    expect(root!.children).toHaveLength(1);
  });
});

describe('computeHighCostThreshold', () => {
  it('returns 0 for empty array', () => {
    expect(computeHighCostThreshold([])).toBe(0);
  });

  it('returns the only value for a single-element array', () => {
    expect(computeHighCostThreshold([42])).toBe(42);
  });

  it('top 20% of five elements is the highest one', () => {
    // 20% of 5 = 1, so threshold = highest cost
    const threshold = computeHighCostThreshold([10, 20, 30, 40, 50]);
    expect(threshold).toBe(50);
  });

  it('top 20% of ten elements is the 2nd highest', () => {
    // 20% of 10 = 2, idx = 1, sorted desc: [100,90,80,...] → threshold = 90
    const costs = [10, 20, 30, 40, 50, 60, 70, 80, 90, 100];
    const threshold = computeHighCostThreshold(costs);
    expect(threshold).toBe(90);
  });
});
