import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';
import { focusTrap } from '$lib/utils/focus-trap';

// Run requestAnimationFrame callbacks synchronously so focus side-effects are
// testable without async flushes.
beforeEach(() => {
  vi.stubGlobal('requestAnimationFrame', (cb: FrameRequestCallback) => {
    cb(0);
    return 0;
  });
});

afterEach(() => {
  vi.unstubAllGlobals();
  // Clear anything appended to body between tests.
  document.body.innerHTML = '';
});

function mount(html: string): HTMLElement {
  const div = document.createElement('div');
  div.innerHTML = html;
  document.body.appendChild(div);
  return div;
}

function tabEvent(shiftKey = false): KeyboardEvent {
  return new KeyboardEvent('keydown', { key: 'Tab', shiftKey, bubbles: true });
}

// ── focusTrap: initial focus ──────────────────────────────────────────────────

describe('focusTrap — initial focus', () => {
  it('moves focus to the first focusable child on mount', () => {
    const container = mount('<button id="b1">First</button><button id="b2">Second</button>');
    focusTrap(container);
    expect(document.activeElement?.id).toBe('b1');
  });

  it('skips disabled buttons when choosing the first focusable child', () => {
    const container = mount(
      '<button id="dis" disabled>Disabled</button><button id="b2">Enabled</button>',
    );
    focusTrap(container);
    expect(document.activeElement?.id).toBe('b2');
  });

  it('does nothing when there are no focusable children', () => {
    const container = mount('<div>No focusable content</div>');
    const previous = document.activeElement;
    focusTrap(container);
    expect(document.activeElement).toBe(previous);
  });
});

// ── focusTrap: Tab wrapping ───────────────────────────────────────────────────

describe('focusTrap — Tab wrapping', () => {
  it('wraps Tab from the last element back to the first', () => {
    const container = mount('<button id="b1">First</button><button id="b2">Last</button>');
    focusTrap(container);
    container.querySelector<HTMLElement>('#b2')!.focus();

    const event = tabEvent();
    const spy = vi.spyOn(event, 'preventDefault');
    container.dispatchEvent(event);

    expect(spy).toHaveBeenCalled();
    expect(document.activeElement?.id).toBe('b1');
  });

  it('wraps Shift+Tab from the first element back to the last', () => {
    const container = mount('<button id="b1">First</button><button id="b2">Last</button>');
    focusTrap(container);
    container.querySelector<HTMLElement>('#b1')!.focus();

    const event = tabEvent(true);
    const spy = vi.spyOn(event, 'preventDefault');
    container.dispatchEvent(event);

    expect(spy).toHaveBeenCalled();
    expect(document.activeElement?.id).toBe('b2');
  });

  it('does not intercept Tab when focus is not at the last element', () => {
    const container = mount(
      '<button id="b1">A</button><button id="b2">B</button><button id="b3">C</button>',
    );
    focusTrap(container);
    container.querySelector<HTMLElement>('#b2')!.focus();

    const event = tabEvent();
    const spy = vi.spyOn(event, 'preventDefault');
    container.dispatchEvent(event);

    expect(spy).not.toHaveBeenCalled();
  });

  it('does not intercept Shift+Tab when focus is not at the first element', () => {
    const container = mount(
      '<button id="b1">A</button><button id="b2">B</button><button id="b3">C</button>',
    );
    focusTrap(container);
    container.querySelector<HTMLElement>('#b2')!.focus();

    const event = tabEvent(true);
    const spy = vi.spyOn(event, 'preventDefault');
    container.dispatchEvent(event);

    expect(spy).not.toHaveBeenCalled();
  });

  it('prevents Tab when there are no focusable children', () => {
    const container = mount('<div>No focusable content</div>');
    focusTrap(container);

    const event = tabEvent();
    const spy = vi.spyOn(event, 'preventDefault');
    container.dispatchEvent(event);

    expect(spy).toHaveBeenCalled();
  });

  it('ignores non-Tab keystrokes', () => {
    const container = mount('<button id="b1">First</button>');
    focusTrap(container);

    const event = new KeyboardEvent('keydown', { key: 'Escape', bubbles: true });
    const spy = vi.spyOn(event, 'preventDefault');
    container.dispatchEvent(event);

    expect(spy).not.toHaveBeenCalled();
  });
});

// ── focusTrap: destroy / cleanup ──────────────────────────────────────────────

describe('focusTrap — destroy', () => {
  it('restores focus to the previously focused element', () => {
    const outside = document.createElement('button');
    outside.id = 'outside';
    document.body.appendChild(outside);
    outside.focus();

    const container = mount('<button id="inner">Inner</button>');
    const trap = focusTrap(container);

    expect(document.activeElement?.id).toBe('inner');
    trap.destroy();
    expect(document.activeElement?.id).toBe('outside');
  });

  it('removes the Tab-trap listener after destroy', () => {
    const container = mount('<button id="b1">First</button><button id="b2">Last</button>');
    const trap = focusTrap(container);
    trap.destroy();

    container.querySelector<HTMLElement>('#b2')!.focus();
    const event = tabEvent();
    const spy = vi.spyOn(event, 'preventDefault');
    container.dispatchEvent(event);

    expect(spy).not.toHaveBeenCalled();
  });
});
