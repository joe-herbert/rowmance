/** Opens a fresh application window. */
import { WebviewWindow } from '@tauri-apps/api/webviewWindow';
import { getCurrentWindow } from '@tauri-apps/api/window';
import { invoke } from '@tauri-apps/api/core';

let windowCount = 0;

/** Repositions native macOS traffic lights to sit centred in the titlebar card.
 *  Reads --panel-spacing from the document root to handle theme switches. */
export function syncTrafficLightPosition(): void {
  const spacingStr = getComputedStyle(document.documentElement)
    .getPropertyValue('--panel-spacing')
    .trim();
  const spacing = spacingStr !== '' ? parseFloat(spacingStr) : 11;
  const cardHeight = 46; // matches .titlebar-card height in AppShell.svelte
  const x = spacing + 16; // 16px inside the card left edge
  const y = spacing + cardHeight / 2 - 3;
  invoke('window_set_traffic_light_position', { x, y }).catch(() => {
    /* non-macOS */
  });
}

/** Notifies `callback` with the current window's fullscreen state, and again on every
 *  change. Returns an unlisten function. No-ops (never calls back) outside Tauri. */
export function onFullscreenChange(callback: (fullscreen: boolean) => void): () => void {
  const win = getCurrentWindow();
  let cancelled = false;
  let unlisten: (() => void) | undefined;

  const check = () => {
    win
      .isFullscreen()
      .then((fullscreen) => {
        if (!cancelled) callback(fullscreen);
      })
      .catch(() => {
        /* non-macOS or not running under Tauri */
      });
  };

  check();
  win
    .onResized(check)
    .then((fn) => {
      if (cancelled) fn();
      else unlisten = fn;
    })
    .catch(() => {});

  return () => {
    cancelled = true;
    unlisten?.();
  };
}

export function openNewWindow(): void {
  const label = `rowmance-${++windowCount}`;
  const win = new WebviewWindow(label, {
    url: '/',
    title: 'Rowmance',
    width: 800,
    height: 600,
    titleBarStyle: 'overlay',
    hiddenTitle: true,
  });
  win.once('tauri://error', (e) => console.error('Failed to create window:', e));
}
