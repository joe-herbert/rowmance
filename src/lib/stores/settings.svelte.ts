/**
 * Application settings store.
 * Settings are persisted in the Rust SQLite backend and mirrored here for
 * synchronous read access throughout the UI.
 */
import { DEFAULT_SETTINGS, type AppSettings } from '$lib/types';
import { getAllSettings, setSetting } from '$lib/tauri/settings';

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

      applyThemeToDocument(settings.theme);
      applyFontToDocument(settings.fontFamily, settings.fontSize);
      loaded = true;
    },

    /** Update a single setting, persisting it to the backend. */
    async set<K extends keyof AppSettings>(key: K, value: AppSettings[K]) {
      settings = { ...settings, [key]: value };

      if (key === 'theme') applyThemeToDocument(value as string);
      if (key === 'fontFamily' || key === 'fontSize') {
        applyFontToDocument(settings.fontFamily, settings.fontSize);
      }

      await setSetting(key, value);
    },
  };
}

// ── Side effects ──────────────────────────────────────────────────────────────

function applyThemeToDocument(theme: string) {
  document.documentElement.setAttribute('data-theme', theme);
  localStorage.setItem('rowmance_theme', theme);
}

function applyFontToDocument(fontFamily: string, fontSize: number) {
  const root = document.documentElement;
  root.style.setProperty('--font-family-ui', fontFamily);
  root.style.setProperty('--font-size-md', `${fontSize}px`);
}
