<!--
  ErdCanvas — renders an Entity-Relationship Diagram for a database.
  Layout is computed in a Web Worker via elkjs.
  Pan and zoom are handled with d3-zoom.
-->
<script lang="ts">
  import { onMount } from 'svelte';
  import { zoom as d3zoom, zoomIdentity, type ZoomBehavior } from 'd3-zoom';
  import { select } from 'd3-selection';
  import ELK from 'elkjs/lib/elk.bundled.js';
  import * as erdApi from '$lib/tauri/erd';
  import { usePanels } from '$lib/stores/panels.svelte';
  import { errorMessage } from '$lib/utils/errors';
  import type { LayoutResult, LayoutEdge } from './erd-worker';

  interface Props {
    connectionId: string;
    database: string;
  }

  const { connectionId, database }: Props = $props();
  const panelStore = usePanels();

  let svgEl = $state<SVGSVGElement | undefined>(undefined);
  let containerEl = $state<HTMLDivElement | undefined>(undefined);

  let layout = $state<LayoutResult | null>(null);
  let loading = $state(true);
  let error = $state<string | null>(null);

  // Current transform for pan/zoom
  let transform = $state({ x: 0, y: 0, k: 1 });

  let zoomBehaviour: ZoomBehavior<SVGSVGElement, unknown> | null = null;

  const NODE_HEADER_HEIGHT = 30;
  const NODE_ROW_HEIGHT = 22;
  const NODE_WIDTH = 200;

  const elk = new ELK();

  onMount(async () => {
    try {
      const graph = await erdApi.getErdGraph(connectionId, database);
      const { nodes, edges } = graph;

      const elkNodes = nodes.map((table) => ({
        id: table.name,
        width: NODE_WIDTH,
        height: NODE_HEADER_HEIGHT + table.columns.length * NODE_ROW_HEIGHT,
      }));

      const elkEdges = edges.map((rel: { constraintName: string; fromTable: string; toTable: string }, i: number) => ({
        id: `edge-${i}-${rel.constraintName}`,
        sources: [rel.fromTable],
        targets: [rel.toTable],
      }));

      const elkGraph = {
        id: 'root',
        layoutOptions: {
          'org.eclipse.elk.algorithm': 'layered',
          'org.eclipse.elk.edgeRouting': 'ORTHOGONAL',
          'org.eclipse.elk.layered.spacing.nodeNodeBetweenLayers': '80',
          'org.eclipse.elk.spacing.nodeNode': '40',
        },
        children: elkNodes,
        edges: elkEdges,
      };

      const layouted: any = await elk.layout(elkGraph);

      const layoutNodes = ((layouted.children ?? []) as any[]).map((n) => {
        const table = nodes.find((t: { name: string }) => t.name === n.id)!;
        return {
          id: n.id,
          x: n.x ?? 0,
          y: n.y ?? 0,
          width: n.width ?? NODE_WIDTH,
          height: n.height ?? NODE_HEADER_HEIGHT + table.columns.length * NODE_ROW_HEIGHT,
          table,
        };
      });

      const layoutEdges = ((layouted.edges ?? []) as any[]).map((e: any, i: number) => ({
        id: e.id,
        relation: edges[i],
        sections: ((e.sections ?? []) as any[]).map((s: any) => ({
          startPoint: s.startPoint ?? { x: 0, y: 0 },
          endPoint: s.endPoint ?? { x: 0, y: 0 },
          bendPoints: s.bendPoints,
        })),
      }));

      layout = {
        nodes: layoutNodes,
        edges: layoutEdges,
        width: layouted.width ?? 800,
        height: layouted.height ?? 600,
      } as LayoutResult;
    } catch (err) {
      error = errorMessage(err);
    }
    loading = false;
  });

  $effect(() => {
    if (!svgEl || !layout) return;

    const svg = select<SVGSVGElement, unknown>(svgEl);
    zoomBehaviour = d3zoom<SVGSVGElement, unknown>()
      .scaleExtent([0.1, 4])
      .on('zoom', (event) => {
        transform = { x: event.transform.x, y: event.transform.y, k: event.transform.k };
      });

    svg.call(zoomBehaviour);

    // Fit the diagram to the viewport on first render.
    const rect = svgEl.getBoundingClientRect();
    if (rect.width > 0 && layout.width > 0 && zoomBehaviour) {
      const scaleX = (rect.width - 80) / layout.width;
      const scaleY = (rect.height - 80) / layout.height;
      const k = Math.min(scaleX, scaleY, 1);
      const tx = (rect.width - layout.width * k) / 2;
      const ty = (rect.height - layout.height * k) / 2;
      svg.call(zoomBehaviour.transform, zoomIdentity.translate(tx, ty).scale(k));
    }

    return () => {
      svg.on('.zoom', null);
    };
  });

  function openTable(tableName: string) {
    panelStore.openInFocused({ kind: 'table_browser', connectionId, database, table: tableName });
  }

  function buildEdgePath(edge: LayoutEdge): string {
    const parts: string[] = [];
    for (const section of edge.sections) {
      const { startPoint, endPoint, bendPoints } = section;
      parts.push(`M ${startPoint.x} ${startPoint.y}`);
      if (bendPoints) {
        for (const bp of bendPoints) {
          parts.push(`L ${bp.x} ${bp.y}`);
        }
      }
      parts.push(`L ${endPoint.x} ${endPoint.y}`);
    }
    return parts.join(' ');
  }

  function exportSvg() {
    if (!svgEl) return;
    const serialiser = new XMLSerializer();
    const svgStr = serialiser.serializeToString(svgEl);
    const blob = new Blob([svgStr], { type: 'image/svg+xml' });
    downloadBlob(blob, `${database}-erd.svg`);
  }

  async function exportPng() {
    if (!svgEl) return;
    const serialiser = new XMLSerializer();
    const svgStr = serialiser.serializeToString(svgEl);
    const blob = new Blob([svgStr], { type: 'image/svg+xml' });
    const url = URL.createObjectURL(blob);
    const img = new Image();
    img.onload = () => {
      const canvas = document.createElement('canvas');
      canvas.width = svgEl!.clientWidth || 1200;
      canvas.height = svgEl!.clientHeight || 800;
      const ctx = canvas.getContext('2d')!;
      ctx.fillStyle = '#fff';
      ctx.fillRect(0, 0, canvas.width, canvas.height);
      ctx.drawImage(img, 0, 0);
      URL.revokeObjectURL(url);
      canvas.toBlob((pngBlob) => {
        if (pngBlob) downloadBlob(pngBlob, `${database}-erd.png`);
      });
    };
    img.src = url;
  }

  function downloadBlob(blob: Blob, filename: string) {
    const a = document.createElement('a');
    a.href = URL.createObjectURL(blob);
    a.download = filename;
    a.click();
    URL.revokeObjectURL(a.href);
  }
</script>

<div class="erd-container" bind:this={containerEl}>
  <div class="erd-toolbar">
    <span class="erd-title">{database} — Entity Relationship Diagram</span>
    <div class="toolbar-gap"></div>
    <button class="toolbar-btn" onclick={exportSvg} title="Export SVG">Export SVG</button>
    <button class="toolbar-btn" onclick={exportPng} title="Export PNG">Export PNG</button>
  </div>

  {#if loading}
    <div class="erd-state">Computing layout…</div>
  {:else if error}
    <div class="erd-state erd-error">{error}</div>
  {:else if layout}
    <svg
      class="erd-svg"
      bind:this={svgEl}
      aria-label="Entity Relationship Diagram for {database}"
      role="img"
    >
      <defs>
        <marker id="arrowhead" markerWidth="8" markerHeight="6" refX="8" refY="3" orient="auto">
          <polygon points="0 0, 8 3, 0 6" fill="var(--color-text-muted)" />
        </marker>
      </defs>
      <g transform="translate({transform.x},{transform.y}) scale({transform.k})">
        <!-- Edges rendered behind nodes -->
        {#each layout.edges as edge (edge.id)}
          <path
            class="erd-edge"
            d={buildEdgePath(edge)}
            marker-end="url(#arrowhead)"
          />
        {/each}

        <!-- Table nodes -->
        {#each layout.nodes as node (node.id)}
          <!-- svelte-ignore a11y_click_events_have_key_events -->
          <g
            class="erd-node"
            transform="translate({node.x},{node.y})"
            onclick={() => openTable(node.id)}
            role="button"
            tabindex="0"
            aria-label="Open table {node.id}"
          >
            <!-- Node background -->
            <rect
              class="node-bg"
              width={NODE_WIDTH}
              height={NODE_HEADER_HEIGHT + node.table.columns.length * NODE_ROW_HEIGHT}
              rx="4"
            />
            <!-- Header -->
            <rect
              class="node-header"
              width={NODE_WIDTH}
              height={NODE_HEADER_HEIGHT}
              rx="4"
            />
            <rect
              class="node-header"
              y="20"
              width={NODE_WIDTH}
              height="10"
            />
            <text
              class="node-title"
              x={NODE_WIDTH / 2}
              y={NODE_HEADER_HEIGHT / 2}
              dominant-baseline="middle"
              text-anchor="middle"
            >{node.id}</text>

            <!-- Columns -->
            {#each node.table.columns as col, idx}
              <g transform="translate(0,{NODE_HEADER_HEIGHT + idx * NODE_ROW_HEIGHT})">
                <rect
                  class="col-row"
                  class:col-row-alt={idx % 2 === 1}
                  width={NODE_WIDTH}
                  height={NODE_ROW_HEIGHT}
                />
                {#if col.isPrimaryKey}
                  <polygon class="col-pk-icon" points="8,3 9.5,7 14,7 10.5,9.5 11.8,14 8,11.5 4.2,14 5.5,9.5 2,7 6.5,7" transform="translate(0, {NODE_ROW_HEIGHT / 2 - 8.5})" />
                {/if}
                <text
                  class="col-name"
                  x={col.isPrimaryKey ? 22 : 8}
                  y={NODE_ROW_HEIGHT / 2}
                  dominant-baseline="middle"
                >{col.name}</text>
                <text
                  class="col-type"
                  x={NODE_WIDTH - 8}
                  y={NODE_ROW_HEIGHT / 2}
                  dominant-baseline="middle"
                  text-anchor="end"
                >{col.dataType}</text>
              </g>
            {/each}
          </g>
        {/each}
      </g>
    </svg>
  {/if}
</div>

<style>
  .erd-container {
    display: flex;
    flex-direction: column;
    height: 100%;
    overflow: hidden;
  }

  .erd-toolbar {
    display: flex;
    align-items: center;
    gap: var(--spacing-2);
    padding: var(--spacing-1) var(--spacing-3);
    border-bottom: 1px solid var(--color-border);
    flex-shrink: 0;
    background: var(--color-bg-secondary);
  }

  .erd-title {
    font-size: var(--font-size-sm);
    font-weight: var(--font-weight-medium);
    color: var(--color-text-primary);
  }

  .toolbar-gap {
    flex: 1;
  }

  .toolbar-btn {
    font-size: var(--font-size-xs);
    padding: 2px var(--spacing-2);
    background: var(--color-bg-secondary);
    border: 1px solid var(--color-border);
    border-radius: var(--radius-sm);
    color: var(--color-text-secondary);
    cursor: pointer;
    transition: background var(--transition-fast);
  }

  .toolbar-btn:hover {
    background: var(--color-bg-hover);
    color: var(--color-text-primary);
  }

  .erd-svg {
    flex: 1;
    width: 100%;
    height: 100%;
    cursor: grab;
  }

  .erd-svg:active {
    cursor: grabbing;
  }

  .erd-state {
    flex: 1;
    display: flex;
    align-items: center;
    justify-content: center;
    font-size: var(--font-size-sm);
    color: var(--color-text-muted);
  }

  .erd-error {
    color: var(--color-danger);
  }

  /* SVG element styles (applied via :global since they live inside <svg>) */
  :global(.erd-edge) {
    fill: none;
    stroke: var(--color-border-strong);
    stroke-width: 1.5px;
  }

  :global(.erd-node) {
    cursor: pointer;
  }

  :global(.node-bg) {
    fill: var(--color-bg-primary);
    stroke: var(--color-border-strong);
    stroke-width: 1px;
    filter: drop-shadow(0 2px 4px rgba(0,0,0,0.08));
  }

  :global(.node-header) {
    fill: var(--color-accent-subtle);
  }

  :global(.node-title) {
    font-size: 12px;
    font-weight: 600;
    fill: var(--color-accent);
    font-family: var(--font-family-ui);
    pointer-events: none;
  }

  :global(.col-row) {
    fill: transparent;
  }

  :global(.col-row-alt) {
    fill: var(--color-table-row-alt);
  }

  :global(.erd-node:hover .node-bg) {
    stroke: var(--color-accent);
    stroke-width: 2px;
  }

  :global(.col-name) {
    font-size: 11px;
    fill: var(--color-text-primary);
    font-family: var(--font-family-mono);
    pointer-events: none;
  }

  :global(.col-type) {
    font-size: 10px;
    fill: var(--color-text-muted);
    font-family: var(--font-family-mono);
    pointer-events: none;
  }

  :global(.col-pk-icon) {
    font-size: 9px;
    fill: var(--color-warning);
    pointer-events: none;
  }
</style>
