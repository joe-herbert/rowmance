<!--
  ReleaseNotes — displays the release notes for a newly installed version.
-->
<script lang="ts">
  import { marked } from 'marked';
  import ActivityIcon from '$lib/components/icons/ActivityIcon.svelte';

  interface Props {
    version: string;
    notes: string;
  }

  const { version, notes }: Props = $props();

  const html = $derived(notes.trim().length > 0 ? (marked(notes) as string) : '');
</script>

<div class="release-notes">
  <div class="release-header">
    <div class="release-icon" aria-hidden="true">
      <ActivityIcon size={28} />
    </div>
    <div>
      <h1 class="release-title">What's New in {version}</h1>
      <p class="release-subtitle">Rowmance was just updated</p>
    </div>
  </div>

  <div class="release-body markdown-body">
    {#if html}
      {@html html}
    {:else}
      <p class="release-empty">
        See the <a
          href="https://github.com/joe-herbert/rowmance/releases/tag/v{version}"
          target="_blank"
          rel="noreferrer">release page</a
        > for what's new.
      </p>
    {/if}
  </div>
</div>

<style>
  .release-notes {
    display: flex;
    flex-direction: column;
    height: 100%;
    padding: 2rem;
    max-width: 680px;
    margin: 0 auto;
    overflow-y: auto;
  }

  .release-header {
    display: flex;
    align-items: center;
    gap: 1rem;
    margin-bottom: 2rem;
  }

  .release-icon {
    flex-shrink: 0;
    width: 52px;
    height: 52px;
    border-radius: var(--radius-xl);
    background: var(--color-accent, #4f46e5);
    color: #fff;
    display: flex;
    align-items: center;
    justify-content: center;
  }

  .release-title {
    font-size: 1.4rem;
    font-weight: 600;
    margin: 0 0 0.2rem;
    color: var(--color-text, inherit);
  }

  .release-subtitle {
    font-size: 0.85rem;
    color: var(--color-text-muted, #888);
    margin: 0;
  }

  .release-empty {
    color: var(--color-text-muted, #888);
  }

  .release-empty a {
    color: var(--color-accent, #4f46e5);
    text-decoration: underline;
  }

  /* GitHub-style markdown rendering */
  :global(.release-body.markdown-body) {
    font-size: 0.9rem;
    line-height: 1.7;
    color: var(--color-text, inherit);
  }

  :global(.release-body.markdown-body h1),
  :global(.release-body.markdown-body h2),
  :global(.release-body.markdown-body h3),
  :global(.release-body.markdown-body h4),
  :global(.release-body.markdown-body h5),
  :global(.release-body.markdown-body h6) {
    font-weight: 600;
    line-height: 1.25;
    margin-top: 1.5rem;
    margin-bottom: 0.5rem;
    color: var(--color-text, inherit);
  }

  :global(.release-body.markdown-body h1) {
    font-size: 1.5rem;
    border-bottom: 1px solid var(--color-border, #e1e4e8);
    padding-bottom: 0.3rem;
  }
  :global(.release-body.markdown-body h2) {
    font-size: 1.25rem;
    border-bottom: 1px solid var(--color-border, #e1e4e8);
    padding-bottom: 0.3rem;
  }
  :global(.release-body.markdown-body h3) {
    font-size: 1.1rem;
  }
  :global(.release-body.markdown-body h4) {
    font-size: 1rem;
  }

  :global(.release-body.markdown-body p) {
    margin: 0 0 1rem;
  }

  :global(.release-body.markdown-body ul),
  :global(.release-body.markdown-body ol) {
    padding-left: 2rem;
    margin: 0 0 1rem;
  }

  :global(.release-body.markdown-body ul) {
    list-style: disc;
  }

  :global(.release-body.markdown-body ol) {
    list-style: decimal;
  }

  :global(.release-body.markdown-body li) {
    margin-bottom: 0.25rem;
  }

  :global(.release-body.markdown-body li + li) {
    margin-top: 0.25rem;
  }

  :global(.release-body.markdown-body code) {
    font-family:
      ui-monospace,
      SFMono-Regular,
      SF Mono,
      Menlo,
      Consolas,
      monospace;
    font-size: 0.85em;
    padding: 0.2em 0.4em;
    background: var(--color-surface-raised, rgba(128, 128, 128, 0.1));
    border-radius: var(--radius-md);
  }

  :global(.release-body.markdown-body pre) {
    padding: 1rem;
    overflow: auto;
    font-size: 0.85em;
    background: var(--color-surface-raised, rgba(128, 128, 128, 0.1));
    border-radius: var(--radius-md);
    margin: 0 0 1rem;
  }

  :global(.release-body.markdown-body pre code) {
    padding: 0;
    background: transparent;
    border-radius: 0;
  }

  :global(.release-body.markdown-body blockquote) {
    padding: 0 1rem;
    color: var(--color-text-muted, #888);
    border-left: 4px solid var(--color-border, #e1e4e8);
    margin: 0 0 1rem;
  }

  :global(.release-body.markdown-body a) {
    color: var(--color-accent, #4f46e5);
    text-decoration: underline;
  }

  :global(.release-body.markdown-body hr) {
    border: none;
    border-top: 1px solid var(--color-border, #e1e4e8);
    margin: 1.5rem 0;
  }

  :global(.release-body.markdown-body table) {
    border-collapse: collapse;
    width: 100%;
    margin: 0 0 1rem;
  }

  :global(.release-body.markdown-body th),
  :global(.release-body.markdown-body td) {
    padding: 0.4rem 0.8rem;
    border: 1px solid var(--color-border, #e1e4e8);
  }

  :global(.release-body.markdown-body th) {
    font-weight: 600;
    background: var(--color-surface-raised, rgba(128, 128, 128, 0.05));
  }

  :global(.release-body.markdown-body img) {
    max-width: 100%;
  }
</style>
