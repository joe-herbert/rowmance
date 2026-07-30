<script lang="ts">
  import { useSettings } from '$lib/stores/settings.svelte';
  import ContextMenu from '$lib/components/ui/ContextMenu.svelte';
  import CtxItem from '$lib/components/ui/CtxItem.svelte';
  import CtxSep from '$lib/components/ui/CtxSep.svelte';
  import CheckIcon from '$lib/components/icons/CheckIcon.svelte';
  import type { StatusBarSegment } from '$lib/types';

  interface Props {
    x: number;
    y: number;
    open: boolean;
    onclose: () => void;
  }

  const { x, y, open, onclose }: Props = $props();

  const settingsStore = useSettings();

  const SEGMENTS: { key: StatusBarSegment; label: string }[] = [
    { key: 'connectionType', label: 'Connection type' },
    { key: 'host', label: 'Host & port' },
    { key: 'unsavedChanges', label: 'Unsaved changes' },
    { key: 'rowCount', label: 'Row count' },
    { key: 'encoding', label: 'Encoding' },
    { key: 'timing', label: 'Query timing' },
  ];

  function isHidden(segment: StatusBarSegment): boolean {
    return settingsStore.settings.statusBarHiddenSegments.includes(segment);
  }

  function toggleSegment(segment: StatusBarSegment) {
    const hidden = settingsStore.settings.statusBarHiddenSegments;
    const next = hidden.includes(segment)
      ? hidden.filter((s) => s !== segment)
      : [...hidden, segment];
    settingsStore.set('statusBarHiddenSegments', next);
  }
</script>

<ContextMenu {x} {y} {open} {onclose} minWidth={190} class="statusbar-ctx-menu">
  {#each SEGMENTS as segment (segment.key)}
    <CtxItem onclick={() => toggleSegment(segment.key)}>
      <span class="ctx-check">
        {#if !isHidden(segment.key)}
          <CheckIcon width={12} height={12} />
        {/if}
      </span>
      {segment.label}
    </CtxItem>
  {/each}
  <CtxSep />
  <CtxItem
    onclick={() => {
      onclose();
      settingsStore.set('statusBarVisible', false);
    }}>Hide status bar</CtxItem
  >
</ContextMenu>

<style>
  .ctx-check {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    width: 14px;
    margin-right: var(--spacing-2);
  }

  :global(.statusbar-ctx-menu:focus) {
    outline: none;
  }

  :global(.statusbar-ctx-menu .ctx-item:focus) {
    outline: none;
  }
</style>
