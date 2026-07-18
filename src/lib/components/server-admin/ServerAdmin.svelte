<!--
  ServerAdmin — top-level server administration panel.
  Reads capability flags from the backend and renders tabs accordingly.
-->
<script lang="ts">
  import { onMount } from 'svelte';
  import type { ServerAdminCapabilityFlags, CapabilityStatus } from '$lib/types';
  import * as api from '$lib/tauri/server_admin';
  import { errorMessage } from '$lib/utils/errors';
  import ProcessList from './ProcessList.svelte';
  import ServerStatus from './ServerStatus.svelte';
  import Variables from './Variables.svelte';
  import Locks from './Locks.svelte';
  import ScheduledJobs from './ScheduledJobs.svelte';
  import InnodbStatus from './InnodbStatus.svelte';
  import VacuumStatus from './VacuumStatus.svelte';
  import Spinner from '$lib/components/ui/Spinner.svelte';

  interface Props {
    connectionId: string;
  }
  const { connectionId }: Props = $props();

  let capabilities = $state<ServerAdminCapabilityFlags | null>(null);
  let loadError = $state<string | null>(null);
  let activeTab = $state<string>('processes');

  onMount(async () => {
    try {
      capabilities = await api.getCapabilities(connectionId);
      // Auto-select first supported tab
      const tabs = visibleTabs();
      if (tabs.length > 0 && !tabs.some(t => t.id === activeTab)) {
        activeTab = tabs[0].id;
      }
    } catch (e) {
      loadError = errorMessage(e);
    }
  });

  function isSupported(s: CapabilityStatus): boolean {
    return s.status === 'supported';
  }

  function visibleTabs() {
    if (!capabilities) return [];
    const tabs: { id: string; label: string; capability: CapabilityStatus }[] = [
      { id: 'processes', label: 'Processes', capability: capabilities.processList },
      { id: 'status', label: 'Status', capability: capabilities.serverStatus },
      { id: 'variables', label: 'Variables', capability: capabilities.variables },
      { id: 'locks', label: 'Locks', capability: capabilities.locks },
      { id: 'jobs', label: 'Jobs', capability: capabilities.scheduledJobs },
      { id: 'innodb', label: 'InnoDB', capability: capabilities.innodbStatus },
      { id: 'vacuum', label: 'Vacuum', capability: capabilities.vacuumStatus },
    ];
    return tabs.filter(t => t.capability.status !== 'notSupported');
  }
</script>

<div class="server-admin">
  <div class="header">
    <span class="title">Server Administration</span>
  </div>

  {#if loadError}
    <div class="error-state">
      <p>Failed to load capabilities: {loadError}</p>
    </div>
  {:else if !capabilities}
    <div class="loading-state">
      <Spinner size={20} />
    </div>
  {:else}
    {@const tabs = visibleTabs()}
    {#if tabs.length === 0}
      <div class="not-supported">
        <p>Server administration is not available for this connection type.</p>
      </div>
    {:else}
      <div class="tab-bar" role="tablist">
        {#each tabs as tab}
          <button
            class="tab-btn"
            class:active={activeTab === tab.id}
            role="tab"
            aria-selected={activeTab === tab.id}
            onclick={() => (activeTab = tab.id)}
          >
            {tab.label}
          </button>
        {/each}
      </div>

      <div class="tab-content">
        {#if activeTab === 'processes'}
          {#if capabilities.processList.status === 'insufficientPrivileges'}
            <div class="capability-notice warning">
              Insufficient privileges to view the process list.
            </div>
          {:else if capabilities.processList.status === 'extensionRequired'}
            <div class="capability-notice">
              Extension required: {(capabilities.processList as { status: 'extensionRequired'; extension: string }).extension}
            </div>
          {:else}
            <ProcessList
              {connectionId}
              canKill={isSupported(capabilities.killSession)}
              canCancel={isSupported(capabilities.cancelSession)}
            />
          {/if}
        {:else if activeTab === 'status'}
          {#if capabilities.serverStatus.status === 'insufficientPrivileges'}
            <div class="capability-notice warning">Insufficient privileges to view server status.</div>
          {:else}
            <ServerStatus {connectionId} />
          {/if}
        {:else if activeTab === 'variables'}
          {#if capabilities.variables.status === 'insufficientPrivileges'}
            <div class="capability-notice warning">Insufficient privileges to view variables.</div>
          {:else}
            <Variables {connectionId} canSet={isSupported(capabilities.setVariable)} />
          {/if}
        {:else if activeTab === 'locks'}
          {#if capabilities.locks.status === 'insufficientPrivileges'}
            <div class="capability-notice warning">Insufficient privileges to view locks.</div>
          {:else}
            <Locks {connectionId} canKill={isSupported(capabilities.killSession)} />
          {/if}
        {:else if activeTab === 'jobs'}
          {#if capabilities.scheduledJobs.status === 'insufficientPrivileges'}
            <div class="capability-notice warning">Insufficient privileges to view scheduled jobs.</div>
          {:else if capabilities.scheduledJobs.status === 'extensionRequired'}
            <div class="capability-notice">
              Extension required: {(capabilities.scheduledJobs as { status: 'extensionRequired'; extension: string }).extension}
            </div>
          {:else}
            <ScheduledJobs {connectionId} />
          {/if}
        {:else if activeTab === 'innodb'}
          <InnodbStatus {connectionId} />
        {:else if activeTab === 'vacuum'}
          <VacuumStatus {connectionId} />
        {/if}
      </div>
    {/if}
  {/if}
</div>

<style>
  .server-admin {
    display: flex;
    flex-direction: column;
    height: 100%;
    overflow: hidden;
    background: var(--color-bg-primary);
  }

  .header {
    padding: 12px 16px 8px;
    border-bottom: 1px solid var(--color-border);
    flex-shrink: 0;
  }

  .title {
    font-size: var(--font-size-sm);
    font-weight: var(--font-weight-semibold);
    color: var(--color-text-primary);
    letter-spacing: 0.01em;
  }

  .loading-state, .error-state, .not-supported {
    flex: 1;
    display: flex;
    align-items: center;
    justify-content: center;
    color: var(--color-text-muted);
    font-size: var(--font-size-sm);
  }

  .tab-bar {
    display: flex;
    gap: 0;
    padding: 0 12px;
    border-bottom: 1px solid var(--color-border);
    flex-shrink: 0;
    overflow-x: auto;
  }

  .tab-btn {
    padding: 8px 14px;
    font-size: var(--font-size-sm);
    color: var(--color-text-muted);
    border-bottom: 2px solid transparent;
    cursor: pointer;
    white-space: nowrap;
    transition: color var(--transition-fast), border-color var(--transition-fast);
    background: transparent;
    border-top: none;
    border-left: none;
    border-right: none;
  }

  .tab-btn:hover {
    color: var(--color-text-primary);
  }

  .tab-btn.active {
    color: var(--color-accent);
    border-bottom-color: var(--color-accent);
    font-weight: var(--font-weight-medium);
  }

  .tab-content {
    flex: 1;
    overflow: hidden;
    display: flex;
    flex-direction: column;
  }

  .capability-notice {
    margin: 16px;
    padding: 12px 16px;
    border-radius: var(--radius-md);
    font-size: var(--font-size-sm);
    background: var(--color-bg-secondary);
    color: var(--color-text-secondary);
    border: 1px solid var(--color-border);
  }

  .capability-notice.warning {
    background: color-mix(in srgb, var(--color-warning, #f59e0b) 10%, transparent);
    border-color: color-mix(in srgb, var(--color-warning, #f59e0b) 30%, transparent);
    color: var(--color-text-primary);
  }
</style>
