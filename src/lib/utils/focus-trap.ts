/**
 * focus-trap — Svelte action that traps keyboard focus within a container.
 *
 * On mount: moves focus to the first focusable child and remembers the
 * previously focused element so it can be restored on destroy.
 */

const FOCUSABLE_SELECTORS = [
  'a[href]',
  'button:not([disabled])',
  'input:not([disabled])',
  'select:not([disabled])',
  'textarea:not([disabled])',
  '[tabindex]:not([tabindex="-1"])',
].join(', ');

function getFocusable(node: HTMLElement): HTMLElement[] {
  return Array.from(node.querySelectorAll<HTMLElement>(FOCUSABLE_SELECTORS)).filter(
    (el) => !el.closest('[hidden]') && getComputedStyle(el).display !== 'none',
  );
}

export function focusTrap(node: HTMLElement) {
  const previouslyFocused = document.activeElement as HTMLElement | null;

  // Focus first focusable child on the next tick so the DOM is settled.
  requestAnimationFrame(() => {
    const focusable = getFocusable(node);
    focusable[0]?.focus();
  });

  function handleKeydown(e: KeyboardEvent) {
    if (e.key !== 'Tab') return;

    const focusable = getFocusable(node);
    if (focusable.length === 0) {
      e.preventDefault();
      return;
    }

    const first = focusable[0];
    const last = focusable[focusable.length - 1];

    if (e.shiftKey) {
      // Shift+Tab — if we're at the first element, wrap to last.
      if (document.activeElement === first) {
        e.preventDefault();
        last.focus();
      }
    } else {
      // Tab — if we're at the last element, wrap to first.
      if (document.activeElement === last) {
        e.preventDefault();
        first.focus();
      }
    }
  }

  node.addEventListener('keydown', handleKeydown);

  return {
    destroy() {
      node.removeEventListener('keydown', handleKeydown);
      previouslyFocused?.focus();
    },
  };
}
