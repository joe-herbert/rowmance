/**
 * Keyboard shortcuts store.
 *
 * Presets define the default bindings for each action. The user can override
 * individual bindings which are persisted to SQLite via the settings backend.
 * `effectiveShortcuts` merges the active preset with stored overrides.
 *
 * A global keydown listener fires a custom "shortcut-action" CustomEvent
 * whenever a registered shortcut is pressed. Components listen for this
 * event instead of registering their own global handlers.
 */
import { invoke } from '@tauri-apps/api/core';

// ── Action names ──────────────────────────────────────────────────────────────

export type ShortcutAction =
  | 'QUERY_RUN_ALL'
  | 'QUERY_RUN_SELECTION'
  | 'QUERY_RUN_UNDER_CURSOR'
  | 'QUERY_FORMAT'
  | 'QUERY_EXPLAIN'
  | 'FOCUS_EDITOR'
  | 'FOCUS_SCHEMA_TREE'
  | 'FOCUS_RESULTS'
  | 'PANEL_NEXT'
  | 'PANEL_PREV'
  | 'PANEL_CLOSE'
  | 'TOGGLE_LEFT_SIDEBAR'
  | 'TOGGLE_RIGHT_SIDEBAR'
  | 'TABLE_SAVE_CHANGES'
  | 'TABLE_DISCARD_CHANGES'
  | 'TABLE_EDIT_IN_MODAL'
  | 'PAGE_NEXT'
  | 'PAGE_PREV'
  | 'COMMAND_PALETTE'
  | 'NEW_QUERY_EDITOR'
  | 'NEW_WINDOW'
  | 'OPEN_SETTINGS';

export interface ShortcutDefinition {
  action: ShortcutAction;
  label: string;
  vscode: string;
  jetbrains: string;
  vim: string;
}

// ── Preset definitions ────────────────────────────────────────────────────────

export const SHORTCUT_DEFINITIONS: ShortcutDefinition[] = [
  {
    action: 'QUERY_RUN_ALL',
    label: 'Run All',
    vscode: 'Mod+Enter',
    jetbrains: 'Mod+Enter',
    vim: 'Mod+Enter',
  },
  {
    action: 'QUERY_RUN_SELECTION',
    label: 'Run Selection',
    vscode: 'Mod+Shift+Enter',
    jetbrains: 'Mod+Shift+Enter',
    vim: 'Mod+Shift+Enter',
  },
  {
    action: 'QUERY_RUN_UNDER_CURSOR',
    label: 'Run Statement Under Cursor',
    vscode: 'Mod+Shift+R',
    jetbrains: 'Mod+Shift+R',
    vim: 'Mod+Shift+R',
  },
  {
    action: 'QUERY_FORMAT',
    label: 'Format Query',
    vscode: 'Mod+Shift+F',
    jetbrains: 'Mod+Alt+L',
    vim: 'Mod+Shift+F',
  },
  {
    action: 'QUERY_EXPLAIN',
    label: 'Explain Query',
    vscode: 'Mod+Shift+E',
    jetbrains: 'Mod+Shift+E',
    vim: 'Mod+Shift+E',
  },
  {
    action: 'FOCUS_EDITOR',
    label: 'Focus Editor',
    vscode: 'Mod+1',
    jetbrains: 'Mod+1',
    vim: 'g e',
  },
  {
    action: 'FOCUS_SCHEMA_TREE',
    label: 'Focus Schema Tree',
    vscode: 'Mod+Shift+E',
    jetbrains: 'Mod+1',
    vim: 'g s',
  },
  {
    action: 'FOCUS_RESULTS',
    label: 'Focus Results',
    vscode: 'Mod+Shift+Y',
    jetbrains: 'Mod+Shift+Y',
    vim: 'g r',
  },
  {
    action: 'PANEL_NEXT',
    label: 'Next Panel',
    vscode: 'Mod+]',
    jetbrains: 'Alt+Right',
    vim: 'g t',
  },
  {
    action: 'PANEL_PREV',
    label: 'Previous Panel',
    vscode: 'Mod+[',
    jetbrains: 'Alt+Left',
    vim: 'g T',
  },
  {
    action: 'PANEL_CLOSE',
    label: 'Close Panel',
    vscode: 'Mod+W',
    jetbrains: 'Mod+F4',
    vim: ':q',
  },
  {
    action: 'TOGGLE_LEFT_SIDEBAR',
    label: 'Toggle Left Sidebar',
    vscode: 'Mod+Shift+B',
    jetbrains: 'Mod+B',
    vim: 'Mod+Shift+B',
  },
  {
    action: 'TOGGLE_RIGHT_SIDEBAR',
    label: 'Toggle Right Sidebar',
    vscode: 'Mod+B',
    jetbrains: 'Mod+Shift+B',
    vim: 'Mod+B',
  },
  {
    action: 'TABLE_SAVE_CHANGES',
    label: 'Save Table Changes',
    vscode: 'Mod+S',
    jetbrains: 'Mod+S',
    vim: ':w',
  },
  {
    action: 'TABLE_DISCARD_CHANGES',
    label: 'Discard Table Changes',
    vscode: 'Escape',
    jetbrains: 'Escape',
    vim: 'u',
  },
  {
    action: 'TABLE_EDIT_IN_MODAL',
    label: 'Edit Cell in Modal',
    vscode: 'Shift+Enter',
    jetbrains: 'Shift+Enter',
    vim: 'Shift+Enter',
  },
  {
    action: 'PAGE_NEXT',
    label: 'Next Page',
    vscode: 'Alt+Right',
    jetbrains: 'Alt+Right',
    vim: 'Ctrl+f',
  },
  {
    action: 'PAGE_PREV',
    label: 'Previous Page',
    vscode: 'Alt+Left',
    jetbrains: 'Alt+Left',
    vim: 'Ctrl+b',
  },
  {
    action: 'COMMAND_PALETTE',
    label: 'Command Palette',
    vscode: 'Mod+K',
    jetbrains: 'Shift+Shift',
    vim: 'Mod+K',
  },
  {
    action: 'NEW_QUERY_EDITOR',
    label: 'New Query Editor',
    vscode: 'Mod+N',
    jetbrains: 'Mod+N',
    vim: 'Mod+N',
  },
  {
    action: 'NEW_WINDOW',
    label: 'New Window',
    vscode: 'Mod+Shift+N',
    jetbrains: 'Mod+Shift+N',
    vim: 'Mod+Shift+N',
  },
  {
    action: 'OPEN_SETTINGS',
    label: 'Open Settings',
    vscode: 'Mod+,',
    jetbrains: 'Mod+,',
    vim: 'Mod+,',
  },
];

// ── State ─────────────────────────────────────────────────────────────────────

type Preset = 'vscode' | 'vim' | 'jetbrains';

let preset = $state<Preset>('vscode');
let overrides = $state<Map<ShortcutAction, string>>(new Map());
let listenerAttached = false;

// ── Derived shortcuts ─────────────────────────────────────────────────────────

function getEffectiveShortcuts(): Map<ShortcutAction, string> {
  const map = new Map<ShortcutAction, string>();
  for (const def of SHORTCUT_DEFINITIONS) {
    const override = overrides.get(def.action);
    map.set(def.action, override ?? def[preset]);
  }
  return map;
}

// ── Keyboard listener ─────────────────────────────────────────────────────────

function keyEventToString(e: KeyboardEvent): string {
  const parts: string[] = [];
  if (e.metaKey || e.ctrlKey) parts.push('Mod');
  if (e.altKey) parts.push('Alt');
  if (e.shiftKey) parts.push('Shift');

  const key = e.key;
  // Normalise to title-case for letter keys.
  if (key.length === 1) {
    parts.push(key.toUpperCase());
  } else {
    parts.push(key);
  }
  return parts.join('+');
}

function attachGlobalListener() {
  if (listenerAttached || typeof window === 'undefined') return;
  listenerAttached = true;

  window.addEventListener('keydown', (e: KeyboardEvent) => {
    const combo = keyEventToString(e);
    const effective = getEffectiveShortcuts();

    for (const [action, shortcut] of effective) {
      if (shortcut === combo) {
        const event = new CustomEvent('shortcut-action', {
          detail: { action },
          bubbles: true,
          cancelable: true,
        });
        const cancelled = !document.dispatchEvent(event);
        if (cancelled) {
          e.preventDefault();
          e.stopPropagation();
        }
        break;
      }
    }
  });
}

// ── Store API ─────────────────────────────────────────────────────────────────

export function useShortcuts() {
  return {
    get preset() {
      return preset;
    },
    get overrides() {
      return overrides;
    },

    get effectiveShortcuts(): Map<ShortcutAction, string> {
      return getEffectiveShortcuts();
    },

    async load(activePreset: Preset) {
      preset = activePreset;

      // Load stored overrides from settings (stored as raw JSON string).
      try {
        const raw = await invoke<string | null>('settings_get', { key: 'keyboard_overrides' });
        if (raw) {
          const parsed = JSON.parse(raw) as Record<string, string>;
          overrides = new Map(Object.entries(parsed) as [ShortcutAction, string][]);
        }
      } catch {
        overrides = new Map();
      }

      attachGlobalListener();
    },

    getShortcut(action: ShortcutAction): string {
      return getEffectiveShortcuts().get(action) ?? '';
    },

    async setOverride(action: ShortcutAction, shortcut: string) {
      const next = new Map(overrides);
      next.set(action, shortcut);
      overrides = next;
      await persistOverrides();
    },

    async clearOverride(action: ShortcutAction) {
      const next = new Map(overrides);
      next.delete(action);
      overrides = next;
      await persistOverrides();
    },
  };
}

async function persistOverrides() {
  const obj: Record<string, string> = {};
  for (const [action, shortcut] of overrides) {
    obj[action] = shortcut;
  }
  await invoke('settings_set', { key: 'keyboard_overrides', value: JSON.stringify(obj) });
}
