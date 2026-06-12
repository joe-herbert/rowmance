/** Opens a fresh application window. */
import { WebviewWindow } from '@tauri-apps/api/webviewWindow';

let windowCount = 0;

export function openNewWindow(): void {
  const label = `rowmance-${++windowCount}`;
  new WebviewWindow(label, {
    url: '/',
    title: 'rowmance',
    width: 800,
    height: 600,
  });
}
