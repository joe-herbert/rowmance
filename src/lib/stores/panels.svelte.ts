/**
 * Panel layout store.
 * Manages the split-panel state for the main area of the application.
 * Up to four panels can be open in a 2×2 grid.
 */
import type { PanelState, PanelKind, SplitMode } from '$lib/types';

// ── Helpers ───────────────────────────────────────────────────────────────────

let nextPanelId = 0;

function createPanelId(): string {
  return `panel-${++nextPanelId}`;
}

function makePanel(content: PanelKind): PanelState {
  return { id: createPanelId(), content };
}

// ── State ─────────────────────────────────────────────────────────────────────

let panels = $state<PanelState[]>([makePanel({ kind: 'empty' })]);
let splitMode = $state<SplitMode>('none');
let focusedIndex = $state(0);

// ── Split mode transitions ────────────────────────────────────────────────────

/**
 * Determines the new SplitMode after adding a panel in the given direction.
 * Follows the state machine defined in the plan:
 *   none + right  → horizontal
 *   none + down   → vertical
 *   horizontal + down → quad
 *   vertical + right  → quad
 */
function nextSplitMode(current: SplitMode, direction: 'right' | 'down'): SplitMode {
  if (current === 'none') return direction === 'right' ? 'horizontal' : 'vertical';
  if (current === 'horizontal' && direction === 'down') return 'quad';
  if (current === 'vertical' && direction === 'right') return 'quad';
  return current;
}

/**
 * Determines the SplitMode after removing a panel.
 *   quad - 1 → horizontal or vertical (depends on remaining layout)
 *   horizontal/vertical - 1 → none
 */
function previousSplitMode(current: SplitMode): SplitMode {
  if (current === 'quad') return 'horizontal'; // caller adjusts as needed
  return 'none';
}

// ── Public interface ──────────────────────────────────────────────────────────

export function usePanels() {
  return {
    get panels() {
      return panels;
    },
    get splitMode() {
      return splitMode;
    },
    get focusedIndex() {
      return focusedIndex;
    },
    get panelCount() {
      return panels.length;
    },
    get focusedPanel(): PanelState {
      return panels[focusedIndex];
    },

    /** Open content in the currently focused panel. */
    openInFocused(content: PanelKind) {
      panels = panels.map((p, i) => (i === focusedIndex ? { ...p, content } : p));
    },

    /** Split the layout by adding a new panel in the given direction. */
    split(direction: 'right' | 'down', content: PanelKind = { kind: 'empty' }) {
      if (panels.length >= 4) return;
      splitMode = nextSplitMode(splitMode, direction);
      panels = [...panels, makePanel(content)];
      focusedIndex = panels.length - 1;
    },

    /** Close the panel at the given index. */
    closePanel(index: number) {
      if (panels.length === 1) {
        // Always keep at least one panel; reset it to empty instead.
        panels = [makePanel({ kind: 'empty' })];
        splitMode = 'none';
        focusedIndex = 0;
        return;
      }

      panels = panels.filter((_, i) => i !== index);
      splitMode = previousSplitMode(splitMode);
      focusedIndex = Math.min(focusedIndex, panels.length - 1);
    },

    /** Move keyboard focus to the next panel (wraps around). */
    focusNext() {
      focusedIndex = (focusedIndex + 1) % panels.length;
    },

    /** Move keyboard focus to the previous panel (wraps around). */
    focusPrev() {
      focusedIndex = (focusedIndex - 1 + panels.length) % panels.length;
    },

    /** Set focus to a specific panel by index. */
    focus(index: number) {
      if (index >= 0 && index < panels.length) {
        focusedIndex = index;
      }
    },
  };
}
