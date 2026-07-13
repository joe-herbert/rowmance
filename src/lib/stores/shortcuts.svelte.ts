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
  | 'TABLE_REFRESH'
  | 'TABLE_SAVE_CHANGES'
  | 'TABLE_DISCARD_CHANGES'
  | 'TABLE_EDIT_IN_MODAL'
  | 'TABLE_SEARCH'
  | 'PAGE_NEXT'
  | 'PAGE_PREV'
  | 'COMMAND_PALETTE'
  | 'NEW_QUERY_EDITOR'
  | 'NEW_WINDOW'
  | 'OPEN_SETTINGS'
  | 'TOGGLE_SYSTEM_ITEMS'
  | 'CLOSE_OTHER_TABS'
  | 'GLOBAL_SEARCH'
  | 'TOGGLE_READ_ONLY'
  | 'TABLE_VIEW_RELATIONS'
  | 'TABLE_QUICK_VIEW_RELATIONS'
  | 'TAB_1'
  | 'TAB_2'
  | 'TAB_3'
  | 'TAB_4'
  | 'TAB_5'
  | 'TAB_6'
  | 'TAB_7'
  | 'TAB_8'
  | 'TAB_9'
  | 'SPLIT_RIGHT'
  | 'SPLIT_DOWN'
  | 'SPLIT_CLOSE'
  | 'SPLIT_FOCUS_NEXT'
  | 'SPLIT_FOCUS_PREV'
  | 'TABLE_CLONE_ROW';

export type ShortcutGroup = 'Query Editor' | 'Navigation' | 'Table View' | 'General';
export type ShortcutLocation = 'global' | 'query-editor' | 'table-view';

export interface ShortcutDefinition {
  action: ShortcutAction;
  label: string;
  group: ShortcutGroup;
  /** Fires regardless of which panel is focused — clashes with any other shortcut. */
  global: boolean;
  /** The UI context where this shortcut is active. */
  location: ShortcutLocation;
  vscode: string;
  jetbrains: string;
  vim: string;
}

// ── Preset definitions ────────────────────────────────────────────────────────

export const SHORTCUT_DEFINITIONS: ShortcutDefinition[] = [
  // ── Query Editor ─────────────────────────────────────────────────────────────
  // Context-specific: only active when the query editor panel is open.
  {
    action: 'QUERY_RUN_ALL',
    label: 'Run All',
    group: 'Query Editor',
    global: false,
    location: 'query-editor',
    vscode: 'Mod+Enter',
    jetbrains: 'Mod+Enter',
    vim: 'Mod+Enter',
  },
  {
    action: 'QUERY_RUN_SELECTION',
    label: 'Run Selection',
    group: 'Query Editor',
    global: false,
    location: 'query-editor',
    vscode: 'Mod+Shift+Enter',
    jetbrains: 'Mod+Shift+Enter',
    vim: 'Mod+Shift+Enter',
  },
  {
    action: 'QUERY_RUN_UNDER_CURSOR',
    label: 'Run Statement Under Cursor',
    group: 'Query Editor',
    global: false,
    location: 'query-editor',
    vscode: 'Mod+Shift+R',
    jetbrains: 'Mod+Shift+R',
    vim: 'Mod+Shift+R',
  },
  {
    action: 'QUERY_FORMAT',
    label: 'Format Query',
    group: 'Query Editor',
    global: false,
    location: 'query-editor',
    vscode: 'Mod+Alt+F',
    jetbrains: 'Mod+Alt+L',
    vim: 'Mod+Alt+F',
  },
  {
    action: 'QUERY_EXPLAIN',
    label: 'Explain Query',
    group: 'Query Editor',
    global: false,
    location: 'query-editor',
    vscode: 'Mod+Shift+X',
    jetbrains: 'Mod+Shift+E',
    vim: 'Mod+Shift+E',
  },
  // ── Navigation ───────────────────────────────────────────────────────────────
  // All navigation shortcuts are global: handled by AppShell with no focus guard,
  // or active across all panel types (FOCUS_EDITOR, FOCUS_RESULTS).
  {
    action: 'FOCUS_EDITOR',
    label: 'Focus Editor',
    group: 'Navigation',
    global: true,
    location: 'global',
    vscode: 'Mod+1',
    jetbrains: 'Mod+1',
    vim: 'g e',
  },
  {
    action: 'FOCUS_SCHEMA_TREE',
    label: 'Focus Schema Tree',
    group: 'Navigation',
    global: true,
    location: 'global',
    vscode: 'Mod+Shift+E',
    jetbrains: 'Mod+Shift+E',
    vim: 'g s',
  },
  {
    action: 'FOCUS_RESULTS',
    label: 'Focus Results',
    group: 'Navigation',
    global: true,
    location: 'global',
    vscode: 'Mod+Shift+Y',
    jetbrains: 'Mod+Shift+Y',
    vim: 'g r',
  },
  {
    action: 'PANEL_NEXT',
    label: 'Next Panel',
    group: 'Navigation',
    global: true,
    location: 'global',
    vscode: 'Mod+]',
    jetbrains: 'Alt+Right',
    vim: 'g t',
  },
  {
    action: 'PANEL_PREV',
    label: 'Previous Panel',
    group: 'Navigation',
    global: true,
    location: 'global',
    vscode: 'Mod+[',
    jetbrains: 'Alt+Left',
    vim: 'g T',
  },
  {
    action: 'PANEL_CLOSE',
    label: 'Close Panel',
    group: 'Navigation',
    global: true,
    location: 'global',
    vscode: 'Mod+W',
    jetbrains: 'Mod+F4',
    vim: ':q',
  },
  {
    action: 'TOGGLE_LEFT_SIDEBAR',
    label: 'Toggle Left Sidebar',
    group: 'Navigation',
    global: true,
    location: 'global',
    vscode: 'Mod+Shift+B',
    jetbrains: 'Mod+B',
    vim: 'Mod+Shift+B',
  },
  {
    action: 'TOGGLE_RIGHT_SIDEBAR',
    label: 'Toggle Right Sidebar',
    group: 'Navigation',
    global: true,
    location: 'global',
    vscode: 'Mod+B',
    jetbrains: 'Mod+Shift+B',
    vim: 'Mod+B',
  },
  {
    action: 'CLOSE_OTHER_TABS',
    label: 'Close Other Tabs',
    group: 'Navigation',
    global: true,
    location: 'global',
    vscode: 'Mod+Shift+W',
    jetbrains: 'Mod+Shift+W',
    vim: 'Mod+Shift+W',
  },
  {
    action: 'TAB_1',
    label: 'Go to Tab 1',
    group: 'Navigation',
    global: true,
    location: 'global',
    vscode: 'Alt+1',
    jetbrains: 'Alt+1',
    vim: 'Alt+1',
  },
  {
    action: 'TAB_2',
    label: 'Go to Tab 2',
    group: 'Navigation',
    global: true,
    location: 'global',
    vscode: 'Alt+2',
    jetbrains: 'Alt+2',
    vim: 'Alt+2',
  },
  {
    action: 'TAB_3',
    label: 'Go to Tab 3',
    group: 'Navigation',
    global: true,
    location: 'global',
    vscode: 'Alt+3',
    jetbrains: 'Alt+3',
    vim: 'Alt+3',
  },
  {
    action: 'TAB_4',
    label: 'Go to Tab 4',
    group: 'Navigation',
    global: true,
    location: 'global',
    vscode: 'Alt+4',
    jetbrains: 'Alt+4',
    vim: 'Alt+4',
  },
  {
    action: 'TAB_5',
    label: 'Go to Tab 5',
    group: 'Navigation',
    global: true,
    location: 'global',
    vscode: 'Alt+5',
    jetbrains: 'Alt+5',
    vim: 'Alt+5',
  },
  {
    action: 'TAB_6',
    label: 'Go to Tab 6',
    group: 'Navigation',
    global: true,
    location: 'global',
    vscode: 'Alt+6',
    jetbrains: 'Alt+6',
    vim: 'Alt+6',
  },
  {
    action: 'TAB_7',
    label: 'Go to Tab 7',
    group: 'Navigation',
    global: true,
    location: 'global',
    vscode: 'Alt+7',
    jetbrains: 'Alt+7',
    vim: 'Alt+7',
  },
  {
    action: 'TAB_8',
    label: 'Go to Tab 8',
    group: 'Navigation',
    global: true,
    location: 'global',
    vscode: 'Alt+8',
    jetbrains: 'Alt+8',
    vim: 'Alt+8',
  },
  {
    action: 'TAB_9',
    label: 'Go to Tab 9 (or Last)',
    group: 'Navigation',
    global: true,
    location: 'global',
    vscode: 'Alt+9',
    jetbrains: 'Alt+9',
    vim: 'Alt+9',
  },
  // ── Split View ───────────────────────────────────────────────────────────────
  {
    action: 'SPLIT_RIGHT',
    label: 'Split Right',
    group: 'Navigation',
    global: true,
    location: 'global',
    vscode: 'Mod+\\',
    jetbrains: 'Mod+\\',
    vim: 'Mod+\\',
  },
  {
    action: 'SPLIT_DOWN',
    label: 'Split Down',
    group: 'Navigation',
    global: true,
    location: 'global',
    vscode: 'Mod+Shift+\\',
    jetbrains: 'Mod+Shift+\\',
    vim: 'Mod+Shift+\\',
  },
  {
    action: 'SPLIT_CLOSE',
    label: 'Close Split',
    group: 'Navigation',
    global: true,
    location: 'global',
    vscode: 'Mod+Shift+K',
    jetbrains: 'Mod+Shift+K',
    vim: 'Mod+Shift+K',
  },
  {
    action: 'SPLIT_FOCUS_NEXT',
    label: 'Focus Next Split',
    group: 'Navigation',
    global: true,
    location: 'global',
    vscode: 'Mod+Alt+]',
    jetbrains: 'Mod+Alt+]',
    vim: 'Mod+Alt+]',
  },
  {
    action: 'SPLIT_FOCUS_PREV',
    label: 'Focus Previous Split',
    group: 'Navigation',
    global: true,
    location: 'global',
    vscode: 'Mod+Alt+[',
    jetbrains: 'Mod+Alt+[',
    vim: 'Mod+Alt+[',
  },
  // ── Table View ───────────────────────────────────────────────────────────────
  // Context-specific: only active when the table browser panel is focused.
  {
    action: 'TABLE_REFRESH',
    label: 'Refresh Table',
    group: 'Table View',
    global: false,
    location: 'table-view',
    vscode: 'Mod+R',
    jetbrains: 'Mod+R',
    vim: 'Mod+R',
  },
  {
    action: 'TABLE_SAVE_CHANGES',
    label: 'Save Table Changes',
    group: 'Table View',
    global: false,
    location: 'table-view',
    vscode: 'Mod+S',
    jetbrains: 'Mod+S',
    vim: ':w',
  },
  {
    action: 'TABLE_DISCARD_CHANGES',
    label: 'Discard Table Changes',
    group: 'Table View',
    global: false,
    location: 'table-view',
    vscode: '',
    jetbrains: '',
    vim: 'u',
  },
  {
    action: 'TABLE_EDIT_IN_MODAL',
    label: 'Edit Cell in Modal',
    group: 'Table View',
    global: false,
    location: 'table-view',
    vscode: 'Shift+Enter',
    jetbrains: 'Shift+Enter',
    vim: 'Shift+Enter',
  },
  {
    action: 'TABLE_SEARCH',
    label: 'Search Table',
    group: 'Table View',
    global: false,
    location: 'table-view',
    vscode: 'Mod+F',
    jetbrains: 'Mod+F',
    vim: 'Mod+F',
  },
  {
    action: 'PAGE_NEXT',
    label: 'Next Page',
    group: 'Table View',
    global: false,
    location: 'table-view',
    vscode: 'Alt+Right',
    jetbrains: 'Alt+PageDown',
    vim: 'Ctrl+f',
  },
  {
    action: 'PAGE_PREV',
    label: 'Previous Page',
    group: 'Table View',
    global: false,
    location: 'table-view',
    vscode: 'Alt+Left',
    jetbrains: 'Alt+PageUp',
    vim: 'Ctrl+b',
  },
  {
    action: 'TABLE_VIEW_RELATIONS',
    label: 'View Relations',
    group: 'Table View',
    global: false,
    location: 'table-view',
    vscode: 'Mod+Enter',
    jetbrains: 'Mod+Enter',
    vim: 'Mod+Enter',
  },
  {
    action: 'TABLE_QUICK_VIEW_RELATIONS',
    label: 'Quick View Relations',
    group: 'Table View',
    global: false,
    location: 'table-view',
    vscode: 'Mod+Shift+Enter',
    jetbrains: 'Mod+Shift+Enter',
    vim: 'Mod+Shift+Enter',
  },
  {
    action: 'TABLE_CLONE_ROW',
    label: 'Clone Row',
    group: 'Table View',
    global: false,
    location: 'table-view',
    vscode: 'Mod+D',
    jetbrains: 'Mod+D',
    vim: 'y r',
  },
  // ── General ──────────────────────────────────────────────────────────────────
  // All general shortcuts are global: handled by AppShell with no focus guard.
  {
    action: 'COMMAND_PALETTE',
    label: 'Command Palette',
    group: 'General',
    global: true,
    location: 'global',
    vscode: 'Mod+K',
    jetbrains: 'Shift+Shift',
    vim: 'Mod+K',
  },
  {
    action: 'NEW_QUERY_EDITOR',
    label: 'New Query Editor',
    group: 'General',
    global: true,
    location: 'global',
    vscode: 'Mod+N',
    jetbrains: 'Mod+N',
    vim: 'Mod+N',
  },
  {
    action: 'NEW_WINDOW',
    label: 'New Window',
    group: 'General',
    global: true,
    location: 'global',
    vscode: 'Mod+Shift+N',
    jetbrains: 'Mod+Shift+N',
    vim: 'Mod+Shift+N',
  },
  {
    action: 'OPEN_SETTINGS',
    label: 'Open Settings',
    group: 'General',
    global: true,
    location: 'global',
    vscode: 'Mod+,',
    jetbrains: 'Mod+,',
    vim: 'Mod+,',
  },
  {
    action: 'TOGGLE_SYSTEM_ITEMS',
    label: 'Toggle System Items',
    group: 'General',
    global: true,
    location: 'global',
    vscode: 'Mod+Shift+H',
    jetbrains: 'Mod+Shift+H',
    vim: 'Mod+Shift+H',
  },
  {
    action: 'GLOBAL_SEARCH',
    label: 'Global Search',
    group: 'General',
    global: true,
    location: 'global',
    vscode: 'Mod+Shift+F',
    jetbrains: 'Mod+Shift+F',
    vim: 'Mod+Shift+F',
  },
  {
    action: 'TOGGLE_READ_ONLY',
    label: 'Toggle Read Only',
    group: 'General',
    global: true,
    location: 'global',
    vscode: 'Mod+Shift+L',
    jetbrains: 'Mod+Shift+L',
    vim: 'Mod+Shift+L',
  },
];

export const isMac =
  typeof navigator !== 'undefined' && navigator.platform.toUpperCase().includes('MAC');

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

export function keyEventToString(e: KeyboardEvent): string {
  const parts: string[] = [];
  if (isMac ? e.metaKey : e.ctrlKey) parts.push('Mod');
  if (isMac ? e.ctrlKey : e.metaKey) parts.push('Ctrl');
  if (e.altKey) parts.push('Alt');
  if (e.shiftKey) parts.push('Shift');

  // On Mac, Alt/Option remaps keys to special characters (Option+1 → ¡).
  // Use the physical key code instead so Alt+1 always means the digit 1.
  let key: string;
  if (e.altKey && e.code) {
    if (e.code.startsWith('Digit')) key = e.code.slice(5);
    else if (e.code.startsWith('Key')) key = e.code.slice(3);
    else key = e.key.length === 1 ? e.key.toUpperCase() : e.key;
  } else {
    key = e.key;
  }
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
