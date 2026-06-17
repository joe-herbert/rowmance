<!--
  ExplainCanvas — visualises the result of EXPLAIN (ANALYZE) as an elkjs node tree.
  High-cost nodes (top 20% of total cost) are highlighted with --color-warning.
  Receives rawJson and dialect as props.
-->
<script lang="ts">
  import { onMount, onDestroy } from 'svelte';
  import { zoom as d3zoom, type ZoomBehavior } from 'd3-zoom';
  import { select } from 'd3-selection';
  import type { ExplainResult } from '$lib/types';

  interface Props {
    rawJson: string;
    dialect: string;
  }

  const { rawJson, dialect }: Props = $props();

  interface ExplainNode {
    id: string;
    nodeType: string;
    relation: string | null;
    estimatedRows: number;
    actualRows: number | null;
    cost: number;
    children: ExplainNode[];
  }

  interface LayoutNode {
    node: ExplainNode;
    x: number;
    y: number;
    width: number;
    height: number;
    isHighCost: boolean;
  }

  interface LayoutEdge {
    x1: number;
    y1: number;
    x2: number;
    y2: number;
  }

  let svgEl = $state<SVGSVGElement | undefined>(undefined);
  let layoutNodes = $state<LayoutNode[]>([]);
  let layoutEdges = $state<LayoutEdge[]>([]);
  let parseError = $state<string | null>(null);
  let transform = $state({ x: 40, y: 40, k: 1 });

  const NODE_WIDTH = 220;
  const NODE_HEIGHT = 70;
  const H_GAP = 60;
  const V_GAP = 40;

  let zoomBehaviour: ZoomBehavior<SVGSVGElement, unknown> | null = null;

  let nodeCounter = 0;
  function nextId() {
    return `n${nodeCounter++}`;
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

  function parseMysql(raw: string): ExplainNode | null {
    try {
      const obj = JSON.parse(raw);
      const queryBlock = obj?.query_block ?? obj;
      return buildNodeFromMysqlBlock(queryBlock);
    } catch {
      return null;
    }
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

  // Simple recursive layout: depth-first, assign (col, row) coordinates.
  function computeLayout(root: ExplainNode): { nodes: LayoutNode[]; edges: LayoutEdge[] } {
    const allNodes: LayoutNode[] = [];
    const allEdges: LayoutEdge[] = [];

    // Collect all costs to determine the threshold for high-cost highlighting.
    const allCosts: number[] = [];
    function collectCosts(n: ExplainNode) {
      allCosts.push(n.cost);
      n.children.forEach(collectCosts);
    }
    collectCosts(root);
    allCosts.sort((a, b) => b - a);
    const top20Idx = Math.max(0, Math.floor(allCosts.length * 0.2) - 1);
    const costThreshold = allCosts[top20Idx] ?? 0;

    // Assign positions with a simple recursive placement.
    let colCursor = 0;

    function place(n: ExplainNode, depth: number): { centerX: number } {
      if (n.children.length === 0) {
        const x = colCursor * (NODE_WIDTH + H_GAP);
        const y = depth * (NODE_HEIGHT + V_GAP);
        colCursor++;
        const layoutNode: LayoutNode = {
          node: n,
          x,
          y,
          width: NODE_WIDTH,
          height: NODE_HEIGHT,
          isHighCost: n.cost >= costThreshold && costThreshold > 0,
        };
        allNodes.push(layoutNode);
        return { centerX: x + NODE_WIDTH / 2 };
      }

      const childCenters: number[] = [];
      for (const child of n.children) {
        childCenters.push(place(child, depth + 1).centerX);
      }

      const minCenter = childCenters[0];
      const maxCenter = childCenters[childCenters.length - 1];
      const ownCenterX = (minCenter + maxCenter) / 2;
      const x = ownCenterX - NODE_WIDTH / 2;
      const y = depth * (NODE_HEIGHT + V_GAP);

      const layoutNode: LayoutNode = {
        node: n,
        x,
        y,
        width: NODE_WIDTH,
        height: NODE_HEIGHT,
        isHighCost: n.cost >= costThreshold && costThreshold > 0,
      };
      allNodes.push(layoutNode);

      for (const center of childCenters) {
        allEdges.push({
          x1: ownCenterX,
          y1: y + NODE_HEIGHT,
          x2: center,
          y2: (depth + 1) * (NODE_HEIGHT + V_GAP),
        });
      }

      return { centerX: ownCenterX };
    }

    place(root, 0);
    return { nodes: allNodes, edges: allEdges };
  }

  $effect(() => {
    nodeCounter = 0;
    let root: ExplainNode | null = null;
    try {
      root = dialect === 'postgres' ? parsePostgres(rawJson) : parseMysql(rawJson);
    } catch (e) {
      parseError = String(e);
      return;
    }
    if (!root) {
      parseError = 'Could not parse EXPLAIN output.';
      return;
    }
    parseError = null;
    const { nodes, edges } = computeLayout(root);
    layoutNodes = nodes;
    layoutEdges = edges;
  });

  $effect(() => {
    if (!svgEl || layoutNodes.length === 0) return;
    const svg = select<SVGSVGElement, unknown>(svgEl);
    zoomBehaviour = d3zoom<SVGSVGElement, unknown>()
      .scaleExtent([0.1, 4])
      .on('zoom', (event) => {
        transform = { x: event.transform.x, y: event.transform.y, k: event.transform.k };
      });
    svg.call(zoomBehaviour);
    return () => {
      svg.on('.zoom', null);
    };
  });

  onDestroy(() => {
    zoomBehaviour = null;
  });
</script>

<div class="explain-container">
  <div class="explain-toolbar">
    <span class="explain-title">EXPLAIN Visualiser</span>
    <span class="dialect-badge">{dialect}</span>
  </div>

  {#if parseError}
    <div class="explain-error">{parseError}</div>
  {:else}
    <svg
      class="explain-svg"
      bind:this={svgEl}
      aria-label="Query execution plan"
      role="img"
    >
      <g transform="translate({transform.x},{transform.y}) scale({transform.k})">
        {#each layoutEdges as edge, i}
          <line
            class="plan-edge"
            x1={edge.x1}
            y1={edge.y1}
            x2={edge.x2}
            y2={edge.y2}
          />
        {/each}

        {#each layoutNodes as n (n.node.id)}
          <g transform="translate({n.x},{n.y})">
            <rect
              class="plan-node"
              class:plan-node-warning={n.isHighCost}
              width={NODE_WIDTH}
              height={NODE_HEIGHT}
              rx="4"
            />
            <text class="plan-node-type" x={NODE_WIDTH / 2} y="20" text-anchor="middle">
              {n.node.nodeType}
            </text>
            {#if n.node.relation}
              <text class="plan-node-relation" x={NODE_WIDTH / 2} y="36" text-anchor="middle">
                {n.node.relation}
              </text>
            {/if}
            <text class="plan-node-meta" x={NODE_WIDTH / 2} y="54" text-anchor="middle">
              {#if n.node.cost > 0}cost: {n.node.cost.toFixed(2)}{/if}
              {#if n.node.actualRows !== null} · rows: {n.node.actualRows}{/if}
            </text>
          </g>
        {/each}
      </g>
    </svg>
  {/if}
</div>

<style>
  .explain-container {
    display: flex;
    flex-direction: column;
    height: 100%;
    overflow: hidden;
  }

  .explain-toolbar {
    display: flex;
    align-items: center;
    gap: var(--spacing-2);
    padding: var(--spacing-1) var(--spacing-3);
    border-bottom: 1px solid var(--color-border);
    flex-shrink: 0;
    background: var(--color-bg-secondary);
  }

  .explain-title {
    font-size: var(--font-size-sm);
    font-weight: var(--font-weight-medium);
    color: var(--color-text-primary);
  }

  .dialect-badge {
    font-size: var(--font-size-xs);
    padding: 1px 6px;
    background: var(--color-accent-subtle);
    color: var(--color-accent);
    border-radius: var(--radius-sm);
    font-weight: var(--font-weight-medium);
  }

  .explain-svg {
    flex: 1;
    width: 100%;
    height: 100%;
    cursor: grab;
  }

  .explain-svg:active {
    cursor: grabbing;
  }

  .explain-error {
    padding: var(--spacing-4);
    color: var(--color-danger);
    font-size: var(--font-size-sm);
  }

  :global(.plan-edge) {
    stroke: var(--color-border-strong);
    stroke-width: 1.5;
  }

  :global(.plan-node) {
    fill: var(--color-bg-primary);
    stroke: var(--color-border-strong);
    stroke-width: 1px;
    filter: drop-shadow(0 1px 3px rgba(0,0,0,0.08));
  }

  :global(.plan-node-warning) {
    fill: var(--color-warning-subtle);
    stroke: var(--color-warning);
  }

  :global(.plan-node-type) {
    font-size: 12px;
    font-weight: 600;
    fill: var(--color-text-primary);
    font-family: var(--font-family-ui);
    pointer-events: none;
  }

  :global(.plan-node-relation) {
    font-size: 11px;
    fill: var(--color-accent);
    font-family: var(--font-family-mono);
    pointer-events: none;
  }

  :global(.plan-node-meta) {
    font-size: 10px;
    fill: var(--color-text-muted);
    font-family: var(--font-family-mono);
    pointer-events: none;
  }
</style>
