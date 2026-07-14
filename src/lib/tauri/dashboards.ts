import { invoke } from '@tauri-apps/api/core';
import type { Dashboard, DashboardWidget } from '$lib/types';

export interface DashboardUpdateInput {
  name: string;
  icon: string;
  pinned: boolean;
  pinnedOrder: number | null;
  widgets: DashboardWidget[];
}

export async function listDashboards(): Promise<Dashboard[]> {
  return invoke<Dashboard[]>('dashboards_list');
}

export async function createDashboard(input: { name: string; icon: string }): Promise<Dashboard> {
  return invoke<Dashboard>('dashboards_create', { input });
}

export async function updateDashboard(id: string, input: DashboardUpdateInput): Promise<Dashboard> {
  return invoke<Dashboard>('dashboards_update', { id, input });
}

export async function deleteDashboard(id: string): Promise<void> {
  return invoke<void>('dashboards_delete', { id });
}
