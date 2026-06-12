/**
 * Web Worker that runs elkjs layout off the main thread.
 * Receives { nodes: ErdTable[], edges: ErdRelation[] } and posts back
 * a layouted graph with x/y coordinates on each node and bendpoints on each edge.
 */
import ELK from 'elkjs/lib/elk.bundled.js';
import type { ErdTable, ErdRelation } from '$lib/types';

const NODE_WIDTH = 200;
const NODE_HEADER_HEIGHT = 30;
const NODE_ROW_HEIGHT = 22;

interface WorkerInput {
  nodes: ErdTable[];
  edges: ErdRelation[];
}

export interface LayoutNode {
  id: string;
  x: number;
  y: number;
  width: number;
  height: number;
  table: ErdTable;
}

export interface BendPoint {
  x: number;
  y: number;
}

export interface LayoutEdge {
  id: string;
  relation: ErdRelation;
  sections: { startPoint: BendPoint; endPoint: BendPoint; bendPoints?: BendPoint[] }[];
}

export interface LayoutResult {
  nodes: LayoutNode[];
  edges: LayoutEdge[];
  width: number;
  height: number;
}

const elk = new ELK();

self.onmessage = async (event: MessageEvent<WorkerInput>) => {
  const { nodes, edges } = event.data;

  const elkNodes = nodes.map((table) => ({
    id: table.name,
    width: NODE_WIDTH,
    height: NODE_HEADER_HEIGHT + table.columns.length * NODE_ROW_HEIGHT,
  }));

  const elkEdges = edges.map((rel, i) => ({
    id: `edge-${i}-${rel.constraintName}`,
    sources: [rel.fromTable],
    targets: [rel.toTable],
  }));

  const graph = {
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

  try {
    // ELK enriches the graph in-place; cast to access layout-output properties.
    // eslint-disable-next-line @typescript-eslint/no-explicit-any
    const layouted: any = await elk.layout(graph);

    const layoutNodes: LayoutNode[] = ((layouted.children ?? []) as any[]).map((n) => {
      const table = nodes.find((t) => t.name === n.id)!;
      return {
        id: n.id,
        x: n.x ?? 0,
        y: n.y ?? 0,
        width: n.width ?? NODE_WIDTH,
        height: n.height ?? NODE_HEADER_HEIGHT + table.columns.length * NODE_ROW_HEIGHT,
        table,
      };
    });

    const layoutEdges: LayoutEdge[] = ((layouted.edges ?? []) as any[]).map((e: any, i: number) => ({
      id: e.id,
      relation: edges[i],
      sections: ((e.sections ?? []) as any[]).map((s: any) => ({
        startPoint: s.startPoint ?? { x: 0, y: 0 },
        endPoint: s.endPoint ?? { x: 0, y: 0 },
        bendPoints: s.bendPoints,
      })),
    }));

    const result: LayoutResult = {
      nodes: layoutNodes,
      edges: layoutEdges,
      width: layouted.width ?? 800,
      height: layouted.height ?? 600,
    };

    self.postMessage({ ok: true, result });
  } catch (err) {
    self.postMessage({ ok: false, error: String(err) });
  }
};
