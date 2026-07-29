import { invoke } from '@tauri-apps/api/core';
import type { Dashboard, DashboardWidget, DashboardVariable } from '$lib/types';

export interface DashboardUpdateInput {
  name: string;
  icon: string;
  color: string | null;
  pinned: boolean;
  pinnedOrder: number | null;
  widgets: DashboardWidget[];
  variables: DashboardVariable[];
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

/** Marks a dashboard as viewed now; returns its last-viewed timestamp from before this call. */
export async function touchViewed(id: string): Promise<string | null> {
  return invoke<string | null>('dashboards_touch_viewed', { id });
}
