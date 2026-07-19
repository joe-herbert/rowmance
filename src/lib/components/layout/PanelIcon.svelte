<script lang="ts">
  import TableIcon from '$lib/components/icons/TableIcon.svelte';
  import TableColumnsIcon from '$lib/components/icons/TableColumnsIcon.svelte';
  import CodeBracketsIcon from '$lib/components/icons/CodeBracketsIcon.svelte';
  import FileDocIcon from '$lib/components/icons/FileDocIcon.svelte';
  import GearIcon from '$lib/components/icons/GearIcon.svelte';
  import PersonIcon from '$lib/components/icons/PersonIcon.svelte';
  import BarChartIcon from '$lib/components/icons/BarChartIcon.svelte';
  import ActivityIcon from '$lib/components/icons/ActivityIcon.svelte';
  import ConnectionsIcon from '$lib/components/icons/ConnectionsIcon.svelte';
  import BoxIcon from '$lib/components/icons/BoxIcon.svelte';
  import { isSystemDatabase, isSystemTable } from '$lib/utils/system-items';
  import { getAllSystemDatabases } from '$lib/stores/dialects.svelte';
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
    system={isSystemDatabase(content.database, [
      ...settingsStore.settings.systemDatabases,
      ...getAllSystemDatabases(),
    ]) || isSystemTable(content.table, settingsStore.settings.systemTablePatterns)}
  />
{:else if content.kind === 'table_structure'}
  <TableColumnsIcon {size} />
{:else if content.kind === 'query_editor'}
  <CodeBracketsIcon {size} />
{:else if content.kind === 'ddl_viewer'}
  <FileDocIcon {size} />
{:else if content.kind === 'settings'}
  <GearIcon width={size} height={size} />
{:else if content.kind === 'user_manager'}
  <PersonIcon {size} />
{:else if content.kind === 'server_admin'}
  <GearIcon width={size} height={size} />
{:else if content.kind === 'speed_analysis'}
  <BarChartIcon {size} />
{:else if content.kind === 'release_notes'}
  <ActivityIcon {size} />
{:else if content.kind === 'connections'}
  <ConnectionsIcon {size} />
{:else if content.kind === 'dashboard'}
  <span class="dash-icon" style="width:{size}px;height:{size}px">
    {@html dashboardsById.get(content.dashboardId)?.icon ?? ''}
  </span>
{:else}
  <BoxIcon {size} />
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
