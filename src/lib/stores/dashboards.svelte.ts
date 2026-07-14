import type { Dashboard, DashboardWidget } from '$lib/types';
import * as api from '$lib/tauri/dashboards';

const LEGACY_KEY = 'rowmance:dashboards';

// ── Position helpers ──────────────────────────────────────────────────────────

function findFreePosition(
  existing: DashboardWidget[],
  w: number,
  h: number,
): { x: number; y: number } {
  for (let y = 1; y < 500; y++) {
    for (let x = 1; x <= 13 - w; x++) {
      const blocked = existing.some(
        (e) => x < e.x + e.w && x + w > e.x && y < e.y + e.h && y + h > e.y,
      );
      if (!blocked) return { x, y };
    }
  }
  return { x: 1, y: 1 };
}

function migratePositions(
  widgets: (Omit<DashboardWidget, 'x' | 'y'> & { x?: number; y?: number })[],
): DashboardWidget[] {
  const placed: DashboardWidget[] = [];
  for (const w of widgets) {
    if (typeof w.x === 'number' && typeof w.y === 'number') {
      placed.push(w as DashboardWidget);
    } else {
      const pos = findFreePosition(placed, w.w, w.h);
      placed.push({ ...w, ...pos } as DashboardWidget);
    }
  }
  return placed;
}

// ── State ─────────────────────────────────────────────────────────────────────

let dashboards = $state<Dashboard[]>([]);
let loaded = $state(false);

// ── Persistence ───────────────────────────────────────────────────────────────

async function persist(id: string) {
  const d = dashboards.find((x) => x.id === id);
  if (!d) return;
  await api.updateDashboard(id, {
    name: d.name,
    icon: d.icon,
    pinned: d.pinned,
    pinnedOrder: d.pinnedOrder,
    widgets: d.widgets,
  });
}

// ── Public interface ──────────────────────────────────────────────────────────

export function useDashboards() {
  return {
    get dashboards() {
      return dashboards;
    },

    get loaded() {
      return loaded;
    },

    get pinned(): Dashboard[] {
      return dashboards
        .filter((d) => d.pinned)
        .sort((a, b) => (a.pinnedOrder ?? 99) - (b.pinnedOrder ?? 99));
    },

    getById(id: string): Dashboard | undefined {
      return dashboards.find((d) => d.id === id);
    },

    async load() {
      let remote = await api.listDashboards();

      // One-time migration from localStorage
      const legacy = localStorage.getItem(LEGACY_KEY);
      if (legacy && remote.length === 0) {
        try {
          const local = JSON.parse(legacy) as Dashboard[];
          for (const d of local) {
            const created = await api.createDashboard({ name: d.name, icon: d.icon });
            await api.updateDashboard(created.id, {
              name: d.name,
              icon: d.icon,
              pinned: d.pinned,
              pinnedOrder: d.pinnedOrder,
              widgets: migratePositions(d.widgets),
            });
          }
          localStorage.removeItem(LEGACY_KEY);
          remote = await api.listDashboards();
        } catch (e) {
          console.warn('Dashboard localStorage migration failed:', e);
        }
      }

      dashboards = remote.map((d) => ({
        ...d,
        widgets: migratePositions(d.widgets),
      }));
      loaded = true;
    },

    async create(input: { name: string; icon: string }): Promise<Dashboard> {
      const d = await api.createDashboard(input);
      dashboards = [...dashboards, { ...d, widgets: [] }];
      return d;
    },

    async delete(id: string) {
      dashboards = dashboards.filter((d) => d.id !== id);
      resequencePinned();
      await api.deleteDashboard(id);
    },

    update(id: string, input: Partial<Pick<Dashboard, 'name' | 'icon' | 'widgets'>>) {
      dashboards = dashboards.map((d) =>
        d.id === id ? { ...d, ...input, updatedAt: new Date().toISOString() } : d,
      );
      void persist(id);
    },

    togglePin(id: string) {
      const dashboard = dashboards.find((d) => d.id === id);
      if (!dashboard) return;

      if (dashboard.pinned) {
        dashboards = dashboards.map((d) =>
          d.id === id ? { ...d, pinned: false, pinnedOrder: null } : d,
        );
        resequencePinned();
      } else {
        const pinnedCount = dashboards.filter((d) => d.pinned).length;
        if (pinnedCount >= 3) return;
        dashboards = dashboards.map((d) =>
          d.id === id ? { ...d, pinned: true, pinnedOrder: pinnedCount } : d,
        );
      }
      void persist(id);
    },

    canPin(): boolean {
      return dashboards.filter((d) => d.pinned).length < 3;
    },

    addWidget(
      dashboardId: string,
      widget: Omit<DashboardWidget, 'id' | 'x' | 'y'>,
    ): DashboardWidget | undefined {
      const existing = dashboards.find((d) => d.id === dashboardId);
      if (!existing) return undefined;
      const pos = findFreePosition(existing.widgets, widget.w, widget.h);
      const newWidget: DashboardWidget = { ...widget, id: crypto.randomUUID(), ...pos };
      dashboards = dashboards.map((d) =>
        d.id === dashboardId
          ? { ...d, widgets: [...d.widgets, newWidget], updatedAt: new Date().toISOString() }
          : d,
      );
      void persist(dashboardId);
      return newWidget;
    },

    updateWidget(
      dashboardId: string,
      widgetId: string,
      input: Partial<Omit<DashboardWidget, 'id'>>,
    ) {
      dashboards = dashboards.map((d) =>
        d.id === dashboardId
          ? {
              ...d,
              widgets: d.widgets.map((w) => (w.id === widgetId ? { ...w, ...input } : w)),
              updatedAt: new Date().toISOString(),
            }
          : d,
      );
      void persist(dashboardId);
    },

    deleteWidget(dashboardId: string, widgetId: string) {
      dashboards = dashboards.map((d) =>
        d.id === dashboardId
          ? {
              ...d,
              widgets: d.widgets.filter((w) => w.id !== widgetId),
              updatedAt: new Date().toISOString(),
            }
          : d,
      );
      void persist(dashboardId);
    },
  };
}

function resequencePinned() {
  const pinned = dashboards
    .filter((d) => d.pinned)
    .sort((a, b) => (a.pinnedOrder ?? 99) - (b.pinnedOrder ?? 99));
  pinned.forEach((d, i) => {
    dashboards = dashboards.map((x) => (x.id === d.id ? { ...x, pinnedOrder: i } : x));
  });
}
