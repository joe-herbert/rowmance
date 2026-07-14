<script lang="ts">
  import TableIcon from '$lib/components/icons/TableIcon.svelte';
  import { isSystemDatabase, isSystemTable } from '$lib/utils/system-items';
  import { useSettings } from '$lib/stores/settings.svelte';
  import { useDashboards } from '$lib/stores/dashboards.svelte';
  import type { PanelKind } from '$lib/types';

  interface Props {
    content: PanelKind;
    size?: number;
  }

  const { content, size = 14 }: Props = $props();

  const settingsStore = useSettings();
  const dashboardsStore = useDashboards();
  const dashboardsById = $derived(new Map(dashboardsStore.dashboards.map((d) => [d.id, d])));
</script>

{#if content.kind === 'table_browser'}
  <TableIcon
    {size}
    system={isSystemDatabase(content.database, settingsStore.settings.systemDatabases) ||
      isSystemTable(content.table, settingsStore.settings.systemTablePatterns)}
  />
{:else if content.kind === 'table_structure'}
  <svg
    width={size}
    height={size}
    viewBox="0 0 24 24"
    fill="none"
    stroke="currentColor"
    stroke-width="1.7"
    stroke-linecap="round"
    stroke-linejoin="round"
  >
    <rect x="3" y="4" width="18" height="16" rx="2"></rect>
    <line x1="9" y1="4" x2="9" y2="20"></line>
    <line x1="15" y1="4" x2="15" y2="20"></line>
  </svg>
{:else if content.kind === 'query_editor'}
  <svg
    width={size}
    height={size}
    viewBox="0 0 24 24"
    fill="none"
    stroke="currentColor"
    stroke-width="1.7"
    stroke-linecap="round"
    stroke-linejoin="round"
  >
    <polyline points="8 7 4 12 8 17"></polyline>
    <polyline points="16 7 20 12 16 17"></polyline>
  </svg>
{:else if content.kind === 'ddl_viewer'}
  <svg
    width={size}
    height={size}
    viewBox="0 0 24 24"
    fill="none"
    stroke="currentColor"
    stroke-width="1.7"
    stroke-linecap="round"
    stroke-linejoin="round"
  >
    <path d="M14 2H6a2 2 0 0 0-2 2v16a2 2 0 0 0 2 2h12a2 2 0 0 0 2-2V8z"></path>
    <polyline points="14 2 14 8 20 8"></polyline>
    <line x1="8" y1="13" x2="16" y2="13"></line>
    <line x1="8" y1="17" x2="13" y2="17"></line>
  </svg>
{:else if content.kind === 'settings'}
  <svg
    width={size}
    height={size}
    viewBox="0 0 24 24"
    fill="none"
    stroke="currentColor"
    stroke-width="1.7"
    stroke-linecap="round"
    stroke-linejoin="round"
  >
    <circle cx="12" cy="12" r="3"></circle>
    <path
      d="M19.4 15a1.65 1.65 0 0 0 .33 1.82l.06.06a2 2 0 0 1-2.83 2.83l-.06-.06a1.65 1.65 0 0 0-1.82-.33 1.65 1.65 0 0 0-1 1.51V21a2 2 0 0 1-4 0v-.09A1.65 1.65 0 0 0 9 19.4a1.65 1.65 0 0 0-1.82.33l-.06.06a2 2 0 0 1-2.83-2.83l.06-.06A1.65 1.65 0 0 0 4.68 15a1.65 1.65 0 0 0-1.51-1H3a2 2 0 0 1 0-4h.09A1.65 1.65 0 0 0 4.6 9a1.65 1.65 0 0 0-.33-1.82l-.06-.06a2 2 0 0 1 2.83-2.83l.06.06A1.65 1.65 0 0 0 9 4.68a1.65 1.65 0 0 0 1-1.51V3a2 2 0 0 1 4 0v.09a1.65 1.65 0 0 0 1 1.51 1.65 1.65 0 0 0 1.82-.33l.06-.06a2 2 0 0 1 2.83 2.83l-.06.06A1.65 1.65 0 0 0 19.4 9a1.65 1.65 0 0 0 1.51 1H21a2 2 0 0 1 0 4h-.09a1.65 1.65 0 0 0-1.51 1z"
    ></path>
  </svg>
{:else if content.kind === 'user_manager'}
  <svg
    width={size}
    height={size}
    viewBox="0 0 24 24"
    fill="none"
    stroke="currentColor"
    stroke-width="1.7"
    stroke-linecap="round"
    stroke-linejoin="round"
  >
    <circle cx="12" cy="8" r="4"></circle>
    <path d="M4 20c0-4 3.6-7 8-7s8 3 8 7"></path>
  </svg>
{:else if content.kind === 'speed_analysis'}
  <svg
    width={size}
    height={size}
    viewBox="0 0 24 24"
    fill="none"
    stroke="currentColor"
    stroke-width="1.7"
    stroke-linecap="round"
    stroke-linejoin="round"
  >
    <line x1="18" y1="20" x2="18" y2="10"></line>
    <line x1="12" y1="20" x2="12" y2="4"></line>
    <line x1="6" y1="20" x2="6" y2="14"></line>
  </svg>
{:else if content.kind === 'release_notes'}
  <svg
    width={size}
    height={size}
    viewBox="0 0 24 24"
    fill="none"
    stroke="currentColor"
    stroke-width="1.7"
    stroke-linecap="round"
    stroke-linejoin="round"
  >
    <polyline points="22 12 18 12 15 21 9 3 6 12 2 12"></polyline>
  </svg>
{:else if content.kind === 'connections'}
  <svg
    width={size}
    height={size}
    viewBox="0 0 24 24"
    fill="none"
    stroke="currentColor"
    stroke-width="1.7"
    stroke-linecap="round"
    stroke-linejoin="round"
  >
    <ellipse cx="12" cy="5" rx="9" ry="3"></ellipse>
    <path d="M21 12c0 1.66-4 3-9 3s-9-1.34-9-3"></path>
    <path d="M3 5v14c0 1.66 4 3 9 3s9-1.34 9-3V5"></path>
  </svg>
{:else if content.kind === 'dashboard'}
  <span class="dash-icon" style="width:{size}px;height:{size}px">
    {@html dashboardsById.get(content.dashboardId)?.icon ?? ''}
  </span>
{:else}
  <svg
    width={size}
    height={size}
    viewBox="0 0 24 24"
    fill="none"
    stroke="currentColor"
    stroke-width="1.7"
    stroke-linecap="round"
  >
    <rect x="3" y="3" width="18" height="18" rx="2"></rect>
  </svg>
{/if}

<style>
  .dash-icon {
    display: flex;
    align-items: center;
  }

  .dash-icon :global(svg) {
    width: 100%;
    height: 100%;
  }
</style>
