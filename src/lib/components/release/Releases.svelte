<!--
  Releases — scrollable list of all GitHub releases with rendered notes.
  Each entry links out to GitHub for downloads.
-->
<script lang="ts">
  import { marked } from 'marked';
  import ActivityIcon from '$lib/components/icons/ActivityIcon.svelte';

  interface GitHubRelease {
    tag_name: string;
    name: string | null;
    body: string | null;
    html_url: string;
    published_at: string | null;
    prerelease: boolean;
  }

  interface ReleaseEntry {
    tag: string;
    title: string;
    html: string;
    url: string;
    date: string;
    prerelease: boolean;
  }

  let releases = $state<ReleaseEntry[]>([]);
  let loading = $state(true);
  let loadError = $state(false);

  function formatDate(iso: string | null): string {
    if (!iso) return '';
    return new Date(iso).toLocaleDateString(undefined, {
      year: 'numeric',
      month: 'long',
      day: 'numeric',
    });
  }

  async function loadReleases() {
    loading = true;
    loadError = false;
    try {
      const r = await fetch(
        'https://api.github.com/repos/joe-herbert/rowmance/releases?per_page=100',
        { headers: { Accept: 'application/vnd.github+json' } },
      );
      if (!r.ok) throw new Error(`GitHub API error: ${r.status}`);
      const data = (await r.json()) as GitHubRelease[];
      releases = data.map((rel) => {
        const body = rel.body?.trim() ?? '';
        return {
          tag: rel.tag_name,
          title: rel.name?.trim() || rel.tag_name,
          html: body.length > 0 ? (marked(body) as string) : '',
          url: rel.html_url,
          date: formatDate(rel.published_at),
          prerelease: rel.prerelease,
        };
      });
    } catch {
      loadError = true;
    } finally {
      loading = false;
    }
  }

  loadReleases();
</script>

<div class="releases">
  <div class="releases-header">
    <div class="releases-icon" aria-hidden="true">
      <ActivityIcon size={28} />
    </div>
    <div>
      <h1 class="releases-title">Releases</h1>
      <p class="releases-subtitle">All Rowmance releases</p>
    </div>
  </div>

  <div class="releases-body">
    {#if loading}
      <p class="releases-status">Loading releases…</p>
    {:else if loadError}
      <p class="releases-status">
        Failed to load releases. See the <a
          href="https://github.com/joe-herbert/rowmance/releases"
          target="_blank"
          rel="noreferrer">releases page</a
        > on GitHub instead.
      </p>
    {:else if releases.length === 0}
      <p class="releases-status">No releases found.</p>
    {:else}
      {#each releases as release (release.tag)}
        <article class="release-entry">
          <header class="release-entry-header">
            <h2 class="release-entry-title">
              {release.title}
              {#if release.prerelease}
                <span class="release-entry-badge">Pre-release</span>
              {/if}
            </h2>
            {#if release.date}
              <span class="release-entry-date">{release.date}</span>
            {/if}
            <a class="release-entry-link" href={release.url} target="_blank" rel="noreferrer"
              >View on GitHub</a
            >
          </header>
          <div class="release-entry-content markdown-body">
            {#if release.html}
              {@html release.html}
            {:else}
              <p class="release-entry-empty">No release notes provided.</p>
            {/if}
          </div>
        </article>
      {/each}
    {/if}
  </div>
</div>

<style>
  .releases {
    display: flex;
    flex-direction: column;
    height: 100%;
    padding: 2rem;
    max-width: 720px;
    margin: 0 auto;
    overflow-y: auto;
  }

  .releases-header {
    display: flex;
    align-items: center;
    gap: 1rem;
    margin-bottom: 2rem;
  }

  .releases-icon {
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

  .releases-title {
    font-size: 1.4rem;
    font-weight: 600;
    margin: 0 0 0.2rem;
    color: var(--color-text, inherit);
  }

  .releases-subtitle {
    font-size: 0.85rem;
    color: var(--color-text-muted, #888);
    margin: 0;
  }

  .releases-body {
    display: flex;
    flex-direction: column;
    gap: 1.75rem;
  }

  .releases-status {
    color: var(--color-text-muted, #888);
  }

  .releases-status a {
    color: var(--color-accent, #4f46e5);
    text-decoration: underline;
  }

  .release-entry {
    padding-bottom: 1.75rem;
    border-bottom: 1px solid var(--color-border, #e1e4e8);
  }

  .release-entry:last-child {
    border-bottom: none;
    padding-bottom: 0;
  }

  .release-entry-header {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 0.6rem;
    margin-bottom: 0.75rem;
  }

  .release-entry-title {
    font-size: 1.1rem;
    font-weight: 600;
    margin: 0;
    color: var(--color-text, inherit);
    display: flex;
    align-items: center;
    gap: 0.5rem;
  }

  .release-entry-badge {
    font-size: 0.7rem;
    font-weight: 500;
    padding: 0.1rem 0.5rem;
    border-radius: var(--radius-sm);
    background: var(--color-surface-raised, rgba(128, 128, 128, 0.15));
    color: var(--color-text-muted, #888);
  }

  .release-entry-date {
    font-size: 0.8rem;
    color: var(--color-text-muted, #888);
  }

  .release-entry-link {
    margin-left: auto;
    font-size: 0.8rem;
    color: var(--color-accent, #4f46e5);
    text-decoration: underline;
    white-space: nowrap;
  }

  .release-entry-empty {
    color: var(--color-text-muted, #888);
  }

  /* GitHub-style markdown rendering */
  :global(.release-entry-content.markdown-body) {
    font-size: 0.9rem;
    line-height: 1.7;
    color: var(--color-text, inherit);
  }

  :global(.release-entry-content.markdown-body h1),
  :global(.release-entry-content.markdown-body h2),
  :global(.release-entry-content.markdown-body h3),
  :global(.release-entry-content.markdown-body h4),
  :global(.release-entry-content.markdown-body h5),
  :global(.release-entry-content.markdown-body h6) {
    font-weight: 600;
    line-height: 1.25;
    margin-top: 1.25rem;
    margin-bottom: 0.5rem;
    color: var(--color-text, inherit);
  }

  :global(.release-entry-content.markdown-body h1) {
    font-size: 1.3rem;
    border-bottom: 1px solid var(--color-border, #e1e4e8);
    padding-bottom: 0.3rem;
  }
  :global(.release-entry-content.markdown-body h2) {
    font-size: 1.15rem;
    border-bottom: 1px solid var(--color-border, #e1e4e8);
    padding-bottom: 0.3rem;
  }
  :global(.release-entry-content.markdown-body h3) {
    font-size: 1.05rem;
  }
  :global(.release-entry-content.markdown-body h4) {
    font-size: 1rem;
  }

  :global(.release-entry-content.markdown-body p) {
    margin: 0 0 1rem;
  }

  :global(.release-entry-content.markdown-body ul),
  :global(.release-entry-content.markdown-body ol) {
    padding-left: 2rem;
    margin: 0 0 1rem;
  }

  :global(.release-entry-content.markdown-body ul) {
    list-style: disc;
  }

  :global(.release-entry-content.markdown-body ol) {
    list-style: decimal;
  }

  :global(.release-entry-content.markdown-body li) {
    margin-bottom: 0.25rem;
  }

  :global(.release-entry-content.markdown-body li + li) {
    margin-top: 0.25rem;
  }

  :global(.release-entry-content.markdown-body code) {
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

  :global(.release-entry-content.markdown-body pre) {
    padding: 1rem;
    overflow: auto;
    font-size: 0.85em;
    background: var(--color-surface-raised, rgba(128, 128, 128, 0.1));
    border-radius: var(--radius-md);
    margin: 0 0 1rem;
  }

  :global(.release-entry-content.markdown-body pre code) {
    padding: 0;
    background: transparent;
    border-radius: 0;
  }

  :global(.release-entry-content.markdown-body blockquote) {
    padding: 0 1rem;
    color: var(--color-text-muted, #888);
    border-left: 4px solid var(--color-border, #e1e4e8);
    margin: 0 0 1rem;
  }

  :global(.release-entry-content.markdown-body a) {
    color: var(--color-accent, #4f46e5);
    text-decoration: underline;
  }

  :global(.release-entry-content.markdown-body hr) {
    border: none;
    border-top: 1px solid var(--color-border, #e1e4e8);
    margin: 1.5rem 0;
  }

  :global(.release-entry-content.markdown-body table) {
    border-collapse: collapse;
    width: 100%;
    margin: 0 0 1rem;
  }

  :global(.release-entry-content.markdown-body th),
  :global(.release-entry-content.markdown-body td) {
    padding: 0.4rem 0.8rem;
    border: 1px solid var(--color-border, #e1e4e8);
  }

  :global(.release-entry-content.markdown-body th) {
    font-weight: 600;
    background: var(--color-surface-raised, rgba(128, 128, 128, 0.05));
  }

  :global(.release-entry-content.markdown-body img) {
    max-width: 100%;
  }
</style>
