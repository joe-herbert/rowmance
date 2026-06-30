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
  | 'OPEN_SETTINGS'
  | 'TOGGLE_SYSTEM_ITEMS'
  | 'CLOSE_OTHER_TABS';

export type ShortcutGroup = 'Query Editor' | 'Navigation' | 'Table View' | 'General';

export interface ShortcutDefinition {
  action: ShortcutAction;
  label: string;
  group: ShortcutGroup;
  vscode: string;
  jetbrains: string;
  vim: string;
}

// ── Preset definitions ────────────────────────────────────────────────────────

export const SHORTCUT_DEFINITIONS: ShortcutDefinition[] = [
  {
    action: 'QUERY_RUN_ALL',
    label: 'Run All',
    group: 'Query Editor',
    vscode: 'Mod+Enter',
    jetbrains: 'Mod+Enter',
    vim: 'Mod+Enter',
  },
  {
    action: 'QUERY_RUN_SELECTION',
    label: 'Run Selection',
    group: 'Query Editor',
    vscode: 'Mod+Shift+Enter',
    jetbrains: 'Mod+Shift+Enter',
    vim: 'Mod+Shift+Enter',
  },
  {
    action: 'QUERY_RUN_UNDER_CURSOR',
    label: 'Run Statement Under Cursor',
    group: 'Query Editor',
    vscode: 'Mod+Shift+R',
    jetbrains: 'Mod+Shift+R',
    vim: 'Mod+Shift+R',
  },
  {
    action: 'QUERY_FORMAT',
    label: 'Format Query',
    group: 'Query Editor',
    vscode: 'Mod+Shift+F',
    jetbrains: 'Mod+Alt+L',
    vim: 'Mod+Shift+F',
  },
  {
    action: 'QUERY_EXPLAIN',
    label: 'Explain Query',
    group: 'Query Editor',
    vscode: 'Mod+Shift+X',
    jetbrains: 'Mod+Shift+E',
    vim: 'Mod+Shift+E',
  },
  {
    action: 'FOCUS_EDITOR',
    label: 'Focus Editor',
    group: 'Navigation',
    vscode: 'Mod+1',
    jetbrains: 'Mod+1',
    vim: 'g e',
  },
  {
    action: 'FOCUS_SCHEMA_TREE',
    label: 'Focus Schema Tree',
    group: 'Navigation',
    vscode: 'Mod+Shift+E',
    jetbrains: 'Alt+1',
    vim: 'g s',
  },
  {
    action: 'FOCUS_RESULTS',
    label: 'Focus Results',
    group: 'Navigation',
    vscode: 'Mod+Shift+Y',
    jetbrains: 'Mod+Shift+Y',
    vim: 'g r',
  },
  {
    action: 'PANEL_NEXT',
    label: 'Next Panel',
    group: 'Navigation',
    vscode: 'Mod+]',
    jetbrains: 'Alt+Right',
    vim: 'g t',
  },
  {
    action: 'PANEL_PREV',
    label: 'Previous Panel',
    group: 'Navigation',
    vscode: 'Mod+[',
    jetbrains: 'Alt+Left',
    vim: 'g T',
  },
  {
    action: 'PANEL_CLOSE',
    label: 'Close Panel',
    group: 'Navigation',
    vscode: 'Mod+W',
    jetbrains: 'Mod+F4',
    vim: ':q',
  },
  {
    action: 'TOGGLE_LEFT_SIDEBAR',
    label: 'Toggle Left Sidebar',
    group: 'Navigation',
    vscode: 'Mod+Shift+B',
    jetbrains: 'Mod+B',
    vim: 'Mod+Shift+B',
  },
  {
    action: 'TOGGLE_RIGHT_SIDEBAR',
    label: 'Toggle Right Sidebar',
    group: 'Navigation',
    vscode: 'Mod+B',
    jetbrains: 'Mod+Shift+B',
    vim: 'Mod+B',
  },
  {
    action: 'TABLE_SAVE_CHANGES',
    label: 'Save Table Changes',
    group: 'Table View',
    vscode: 'Mod+S',
    jetbrains: 'Mod+S',
    vim: ':w',
  },
  {
    action: 'TABLE_DISCARD_CHANGES',
    label: 'Discard Table Changes',
    group: 'Table View',
    vscode: '',
    jetbrains: '',
    vim: 'u',
  },
  {
    action: 'TABLE_EDIT_IN_MODAL',
    label: 'Edit Cell in Modal',
    group: 'Table View',
    vscode: 'Shift+Enter',
    jetbrains: 'Shift+Enter',
    vim: 'Shift+Enter',
  },
  {
    action: 'PAGE_NEXT',
    label: 'Next Page',
    group: 'Table View',
    vscode: 'Alt+Right',
    jetbrains: 'Alt+PageDown',
    vim: 'Ctrl+f',
  },
  {
    action: 'PAGE_PREV',
    label: 'Previous Page',
    group: 'Table View',
    vscode: 'Alt+Left',
    jetbrains: 'Alt+PageUp',
    vim: 'Ctrl+b',
  },
  {
    action: 'COMMAND_PALETTE',
    label: 'Command Palette',
    group: 'General',
    vscode: 'Mod+K',
    jetbrains: 'Shift+Shift',
    vim: 'Mod+K',
  },
  {
    action: 'NEW_QUERY_EDITOR',
    label: 'New Query Editor',
    group: 'General',
    vscode: 'Mod+N',
    jetbrains: 'Mod+N',
    vim: 'Mod+N',
  },
  {
    action: 'NEW_WINDOW',
    label: 'New Window',
    group: 'General',
    vscode: 'Mod+Shift+N',
    jetbrains: 'Mod+Shift+N',
    vim: 'Mod+Shift+N',
  },
  {
    action: 'OPEN_SETTINGS',
    label: 'Open Settings',
    group: 'General',
    vscode: 'Mod+,',
    jetbrains: 'Mod+,',
    vim: 'Mod+,',
  },
  {
    action: 'TOGGLE_SYSTEM_ITEMS',
    label: 'Toggle System Items',
    group: 'General',
    vscode: 'Mod+Shift+H',
    jetbrains: 'Mod+Shift+H',
    vim: 'Mod+Shift+H',
  },
  {
    action: 'CLOSE_OTHER_TABS',
    label: 'Close Other Tabs',
    group: 'Navigation',
    vscode: 'Mod+Shift+W',
    jetbrains: 'Mod+Shift+W',
    vim: 'Mod+Shift+W',
  },
];

export const isMac = typeof navigator !== 'undefined' && navigator.platform.toUpperCase().includes('MAC');

// ── State ─────────────────────────────────────────────────────────────────────

type Preset = 'vscode' | 'vim' | 'jetbrains';

let preset = $state<Preset>('vscode');
let overrides = $state<Map<ShortcutAction, string>>(new Map());
let listenerAttached = false;
let recordingActive = false;

// ── Derived shortcuts ─────────────────────────────────────────────────────────

function getEffectiveShortcuts(): Map<ShortcutAction, string> {
  const map = new Map<ShortcutAction, string>();
  for (const def of SHORTCUT_DEFINITIONS) {
    const override = overrides.get(def.action);
    const binding = override ?? def[preset];
    if (binding) map.set(def.action, binding);
  }
  return map;
}

// ── Keyboard listener ─────────────────────────────────────────────────────────

const MODIFIER_KEYS = new Set(['Meta', 'Control', 'Alt', 'Shift']);
const DOUBLE_TAP_MS = 400;

function keyEventToString(e: KeyboardEvent): string {
  const parts: string[] = [];
  if (isMac ? e.metaKey : e.ctrlKey) parts.push('Mod');
  if (isMac ? e.ctrlKey : e.metaKey) parts.push('Ctrl');
  if (e.altKey) parts.push('Alt');
  if (e.shiftKey) parts.push('Shift');

  const key = e.key;
  if (!MODIFIER_KEYS.has(key)) {
    parts.push(key.length === 1 ? key.toUpperCase() : key);
  }
  return parts.join('+');
}

function attachGlobalListener() {
  if (listenerAttached || typeof window === 'undefined') return;
  listenerAttached = true;

  let lastCombo = '';
  let lastComboTime = 0;

  window.addEventListener('keydown', (e: KeyboardEvent) => {
    if (recordingActive) return;

    const now = Date.now();
    const combo = keyEventToString(e);
    const effective = getEffectiveShortcuts();

    // Check for double-tap bindings (e.g. Shift+Shift) before regular combos.
    if (combo && combo === lastCombo && now - lastComboTime < DOUBLE_TAP_MS) {
      const doubleTap = `${combo}+${combo}`;
      for (const [action, shortcut] of effective) {
        if (shortcut === doubleTap) {
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
          lastCombo = '';
          lastComboTime = 0;
          return;
        }
      }
    }

    lastCombo = combo;
    lastComboTime = now;

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

    setPreset(p: Preset) {
      preset = p;
    },

    get isRecording() {
      return recordingActive;
    },

    setRecording(active: boolean) {
      recordingActive = active;
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
