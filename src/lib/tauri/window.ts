/** Opens a fresh application window. */
import { WebviewWindow } from '@tauri-apps/api/webviewWindow';
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
