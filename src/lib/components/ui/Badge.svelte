<script lang="ts">
  import type { Snippet } from 'svelte';

  interface Props {
    variant?: 'accent' | 'success' | 'neutral';
    uppercase?: boolean;
    children: Snippet;
  }

  let { variant = 'neutral', uppercase = false, children }: Props = $props();
</script>

<span class="badge badge--{variant}" class:uppercase>{@render children()}</span>

<style>
  .badge {
    display: inline-flex;
    align-items: center;
    padding: 1px 6px;
    border-radius: var(--radius-sm);
    font-family: var(--font-family-mono);
    font-size: 10px;
    font-weight: var(--font-weight-semibold);
    line-height: 1.6;
  }

  .badge.uppercase {
    text-transform: uppercase;
    letter-spacing: 0.05em;
  }

  .badge--accent {
    background: color-mix(in srgb, var(--color-accent) 15%, transparent);
    color: var(--color-accent);
  }

  .badge--success {
    background: var(--color-success-subtle);
    color: var(--color-success);
  }

  .badge--neutral {
    background: var(--color-bg-secondary);
    color: var(--color-text-muted);
  }
</style>
