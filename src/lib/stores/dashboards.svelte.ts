import type { Dashboard, DashboardWidget } from '$lib/types';

const STORAGE_KEY = 'rowmance:dashboards';

function generateId(): string {
  return crypto.randomUUID();
}

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

function loadFromStorage(): Dashboard[] {
  try {
    const stored = localStorage.getItem(STORAGE_KEY);
    if (!stored) return [];
    const parsed = JSON.parse(stored) as Dashboard[];
    return parsed.map((d) => ({ ...d, widgets: migratePositions(d.widgets) }));
  } catch {
    return [];
  }
}

function saveToStorage(list: Dashboard[]) {
  localStorage.setItem(STORAGE_KEY, JSON.stringify(list));
}

let dashboards = $state<Dashboard[]>(loadFromStorage());

export function useDashboards() {
  return {
    get dashboards() {
      return dashboards;
    },

    get pinned(): Dashboard[] {
      return dashboards
        .filter((d) => d.pinned)
        .sort((a, b) => (a.pinnedOrder ?? 99) - (b.pinnedOrder ?? 99));
    },

    getById(id: string): Dashboard | undefined {
      return dashboards.find((d) => d.id === id);
    },

    create(input: { name: string; icon: string }): Dashboard {
      const now = new Date().toISOString();
      const dashboard: Dashboard = {
        id: generateId(),
        name: input.name,
        icon: input.icon,
        pinned: false,
        pinnedOrder: null,
        widgets: [],
        createdAt: now,
        updatedAt: now,
      };
      dashboards = [...dashboards, dashboard];
      saveToStorage(dashboards);
      return dashboard;
    },

    update(id: string, input: Partial<Pick<Dashboard, 'name' | 'icon' | 'widgets'>>) {
      dashboards = dashboards.map((d) =>
        d.id === id ? { ...d, ...input, updatedAt: new Date().toISOString() } : d,
      );
      saveToStorage(dashboards);
    },

    delete(id: string) {
      dashboards = dashboards.filter((d) => d.id !== id);
      resequencePinned();
      saveToStorage(dashboards);
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
      saveToStorage(dashboards);
    },

    canPin(): boolean {
      return dashboards.filter((d) => d.pinned).length < 3;
    },

    addWidget(
      dashboardId: string,
      widget: Omit<DashboardWidget, 'id' | 'x' | 'y'>,
    ): DashboardWidget {
      const existing = dashboards.find((d) => d.id === dashboardId)?.widgets ?? [];
      const pos = findFreePosition(existing, widget.w, widget.h);
      const newWidget: DashboardWidget = { ...widget, id: generateId(), ...pos };
      dashboards = dashboards.map((d) =>
        d.id === dashboardId
          ? { ...d, widgets: [...d.widgets, newWidget], updatedAt: new Date().toISOString() }
          : d,
      );
      saveToStorage(dashboards);
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
      saveToStorage(dashboards);
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
      saveToStorage(dashboards);
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
