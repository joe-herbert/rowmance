<!--
  ReleaseNotes — displays the release notes for a newly installed version.
-->
<script lang="ts">
  interface Props {
    version: string;
    notes: string;
  }

  const { version, notes }: Props = $props();

  const paragraphs = $derived(notes.split(/\n+/).filter((p) => p.trim().length > 0));
</script>

<div class="release-notes">
  <div class="release-header">
    <div class="release-icon" aria-hidden="true">
      <svg
        width="28"
        height="28"
        viewBox="0 0 24 24"
        fill="none"
        stroke="currentColor"
        stroke-width="1.5"
        stroke-linecap="round"
        stroke-linejoin="round"
      >
        <polyline points="22 12 18 12 15 21 9 3 6 12 2 12"></polyline>
      </svg>
    </div>
    <div>
      <h1 class="release-title">What's New in {version}</h1>
      <p class="release-subtitle">Rowmance was just updated</p>
    </div>
  </div>

  <div class="release-body">
    {#each paragraphs as paragraph}
      <p>{paragraph}</p>
    {/each}
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
    border-radius: 12px;
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

  .release-body {
    font-size: 0.9rem;
    line-height: 1.7;
    color: var(--color-text, inherit);
  }

  .release-body p {
    margin: 0 0 0.75rem;
  }
</style>
