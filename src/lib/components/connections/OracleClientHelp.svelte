<!--
  OracleClientHelp — instructions for installing the Oracle Instant Client,
  shown when a connection attempt fails with DPI-1047 (client library not found).
-->
<script lang="ts">
  import InfoCircleIcon from '$lib/components/icons/InfoCircleIcon.svelte';
  import CodeBlock from './CodeBlock.svelte';

  type Os = 'mac' | 'windows' | 'linux';

  function detectOs(): Os {
    const platform = navigator.platform;
    if (/mac/i.test(platform)) return 'mac';
    if (/win/i.test(platform)) return 'windows';
    return 'linux';
  }

  let activeOs = $state<Os>(detectOs());

  const tabs: { id: Os; label: string }[] = [
    { id: 'mac', label: 'macOS' },
    { id: 'windows', label: 'Windows' },
    { id: 'linux', label: 'Linux' },
  ];
</script>

<div class="help">
  <div class="help-header">
    <div class="help-icon" aria-hidden="true">
      <InfoCircleIcon width={28} height={28} />
    </div>
    <div>
      <h1 class="help-title">Install the Oracle Instant Client</h1>
      <p class="help-subtitle">
        Rowmance needs Oracle's client library installed locally to connect to Oracle databases.
      </p>
    </div>
  </div>

  <div class="help-tabs" role="tablist">
    {#each tabs as tab (tab.id)}
      <button
        type="button"
        role="tab"
        aria-selected={activeOs === tab.id}
        class="help-tab"
        class:active={activeOs === tab.id}
        onclick={() => (activeOs = tab.id)}
      >
        {tab.label}
      </button>
    {/each}
  </div>

  <div class="help-body">
    {#if activeOs === 'mac'}
      <ol>
        <li>
          Download the <strong>Basic</strong> Instant Client package for your Mac's architecture
          (arm64 for Apple Silicon, x86-64 for Intel) from Oracle's Instant Client downloads page.
        </li>
        <li>
          Unzip it somewhere stable, e.g.:
          <CodeBlock
            code={`sudo mkdir -p /opt/oracle
sudo mv ~/Downloads/instantclient_* /opt/oracle/`}
          />
        </li>
        <li>
          Symlink the client libraries into <code>/usr/local/lib</code> so macOS can find them:
          <CodeBlock
            code={`cd /opt/oracle/instantclient_*
for f in *.dylib; do sudo ln -sf "$(pwd)/$f" "/usr/local/lib/$f"; done`}
          />
        </li>
        <li>Restart Rowmance and try connecting again.</li>
      </ol>
    {:else if activeOs === 'windows'}
      <ol>
        <li>
          Download the <strong>Basic</strong> Instant Client zip for Windows (64-bit) from Oracle's
          Instant Client downloads page.
        </li>
        <li>
          Unzip it somewhere stable, e.g. <code>C:\oracle\instantclient_XX_X</code>.
        </li>
        <li>
          Add that folder to your <code>PATH</code> environment variable (Settings → System →
          About → Advanced system settings → Environment Variables), then restart Rowmance so it
          picks up the change.
        </li>
        <li>
          If Windows reports a missing Visual C++ runtime, install the latest
          Microsoft Visual C++ Redistributable as noted on the Instant Client download page.
        </li>
      </ol>
    {:else}
      <ol>
        <li>
          Download the <strong>Basic</strong> Instant Client zip for Linux (matching your
          architecture) from Oracle's Instant Client downloads page.
        </li>
        <li>
          Unzip it somewhere stable, e.g.:
          <CodeBlock
            code={`sudo mkdir -p /opt/oracle
sudo unzip instantclient-basic-linux.zip -d /opt/oracle`}
          />
        </li>
        <li>
          Point the dynamic linker at it, either by adding it to <code>ldconfig</code>:
          <CodeBlock
            code={`sudo sh -c "echo /opt/oracle/instantclient_* > /etc/ld.so.conf.d/oracle-instantclient.conf"
sudo ldconfig`}
          />
          or by exporting <code>LD_LIBRARY_PATH</code> before launching Rowmance.
        </li>
        <li>Restart Rowmance and try connecting again.</li>
      </ol>
    {/if}

    <p class="help-note">
      This is a one-time setup on this machine — Oracle's license doesn't allow the client library
      to be bundled inside Rowmance, so it has to be installed separately. It's free to download.
    </p>
  </div>
</div>

<style>
  .help {
    display: flex;
    flex-direction: column;
    width: 100%;
    min-width: 0;
    height: 100%;
    padding: 2rem;
    max-width: 680px;
    margin: 0 auto;
    overflow-y: auto;
    box-sizing: border-box;
  }

  .help-header {
    display: flex;
    align-items: center;
    gap: 1rem;
    margin-bottom: 1.5rem;
    min-width: 0;
  }

  .help-header > div {
    min-width: 0;
  }

  .help-icon {
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

  .help-title {
    font-size: 1.4rem;
    font-weight: 600;
    margin: 0 0 0.2rem;
    color: var(--color-text, inherit);
  }

  .help-subtitle {
    font-size: 1rem;
    color: var(--color-text-muted, #888);
    margin: 0;
  }

  .help-tabs {
    display: flex;
    gap: 0.25rem;
    border-bottom: 1px solid var(--color-border, #e1e4e8);
    margin-bottom: 1.5rem;
  }

  .help-tab {
    appearance: none;
    background: none;
    border: none;
    border-bottom: 2px solid transparent;
    padding: 0.6rem 1rem;
    font-size: 1rem;
    font-weight: 500;
    color: var(--color-text-muted, #888);
    cursor: pointer;
  }

  .help-tab:hover {
    color: var(--color-text, inherit);
  }

  .help-tab.active {
    color: var(--color-accent, #4f46e5);
    border-bottom-color: var(--color-accent, #4f46e5);
  }

  .help-body {
    font-size: 1.05rem;
    line-height: 1.7;
    color: var(--color-text, inherit);
  }

  .help-body ol {
    padding-left: 1.25rem;
    margin: 0 0 1rem;
  }

  .help-body li {
    margin-bottom: 0.75rem;
  }

  .help-body code {
    overflow-wrap: anywhere;
    font-family:
      ui-monospace,
      SFMono-Regular,
      SF Mono,
      Menlo,
      Consolas,
      monospace;
    font-size: 0.9em;
    padding: 0.2em 0.4em;
    background: var(--color-surface-raised, rgba(128, 128, 128, 0.1));
    border-radius: var(--radius-md);
  }

  .help-title,
  .help-subtitle,
  .help-body,
  .help-body :global(*) {
    -webkit-user-select: text;
    user-select: text;
  }

  .help-body code {
    cursor: text;
  }

  .help-note {
    color: var(--color-text-muted, #888);
    font-size: 0.9rem;
    margin-top: 1.5rem;
  }
</style>
