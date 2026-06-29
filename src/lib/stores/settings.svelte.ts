/**
 * Application settings store.
 * Settings are persisted in the Rust SQLite backend and mirrored here for
 * synchronous read access throughout the UI.
 */
import { DEFAULT_SETTINGS, type AppSettings } from '$lib/types';
import { getAllSettings, setSetting } from '$lib/tauri/settings';
import { themesRead } from '$lib/tauri/themes';
import { syncTrafficLightPosition } from '$lib/tauri/window';

// ── State ─────────────────────────────────────────────────────────────────────

let settings = $state<AppSettings>({ ...DEFAULT_SETTINGS });
let loaded = $state(false);

// ── Public interface ──────────────────────────────────────────────────────────

export function useSettings() {
  return {
    get settings() {
      return settings;
    },
    get loaded() {
      return loaded;
    },

    /** Load all settings from the backend. Call once at app startup. */
    async load() {
      try {
        const persisted = await getAllSettings();
        settings = { ...DEFAULT_SETTINGS, ...persisted };
      } catch {
        // Backend not available during tests — use defaults.
        settings = { ...DEFAULT_SETTINGS };
      }

      await applyTheme(settings.theme);
      applyFontToDocument(settings.fontFamily, settings.fontSize);
      syncTrafficLightPosition();
      loaded = true;
    },

    /** Update a single setting, persisting it to the backend. */
    async set<K extends keyof AppSettings>(key: K, value: AppSettings[K]) {
      settings = { ...settings, [key]: value };

      if (key === 'theme') {
        await applyTheme(value as string);
        applyFontToDocument(settings.fontFamily, settings.fontSize);
        syncTrafficLightPosition();
      }
      if (key === 'fontFamily' || key === 'fontSize') {
        applyFontToDocument(settings.fontFamily, settings.fontSize);
      }

      await setSetting(key, value);
    },
  };
}

// ── Side effects ──────────────────────────────────────────────────────────────

const BUILTIN_THEMES = new Set(['light', 'dark', 'system', 'high-contrast']);

function clearCustomThemeStyles() {
  const style = document.documentElement.style;
  const toRemove: string[] = [];
  for (let i = 0; i < style.length; i++) {
    if (style[i].startsWith('--')) toRemove.push(style[i]);
  }
  toRemove.forEach((p) => style.removeProperty(p));
}

async function applyTheme(theme: string) {
  clearCustomThemeStyles();
  document.documentElement.setAttribute('data-theme', theme);
  localStorage.setItem('rowmance_theme', theme);
  if (!BUILTIN_THEMES.has(theme)) {
    try {
      const data = await themesRead(theme);
      for (const [k, v] of Object.entries(data.variables)) {
        document.documentElement.style.setProperty(k, v);
      }
    } catch {
      // Custom theme not found — fall back to system without looping.
      document.documentElement.setAttribute('data-theme', 'system');
      localStorage.setItem('rowmance_theme', 'system');
    }
  }
}

function applyFontToDocument(fontFamily: string, fontSize: number) {
  const root = document.documentElement;
  root.style.setProperty('--font-family-ui', fontFamily);
  root.style.setProperty('--font-size-md', `${fontSize}px`);
}
