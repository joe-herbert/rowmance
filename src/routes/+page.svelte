<script lang="ts">
  import '../styles/variables.css';
  import '../styles/themes/light.css';
  import '../styles/themes/dark.css';
  import '../styles/themes/high-contrast.css';
  import '../styles/themes/ssms.css';
  import '../styles/base.css';
  import AppShell from '$lib/components/layout/AppShell.svelte';
  import { useSettings } from '$lib/stores/settings.svelte';
  import { useConnections } from '$lib/stores/connections.svelte';

  const settingsStore = useSettings();
  const connectionStore = useConnections();

  $effect(() => {
    settingsStore.load();
    connectionStore.load();
  });
</script>

{#if settingsStore.loaded}
  <AppShell />
{:else}
  <div class="loading-screen">
    <span>Loading…</span>
  </div>
{/if}

<style>
  .loading-screen {
    display: flex;
    align-items: center;
    justify-content: center;
    height: 100vh;
    color: var(--color-text-muted);
    font-size: var(--font-size-md);
  }
</style>
