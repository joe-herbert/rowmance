import { describe, it, expect, vi, beforeEach } from 'vitest';

// Re-import after each reset so module-level $state is re-initialised.
let useToast: typeof import('./toast.svelte').useToast;

beforeEach(async () => {
  vi.resetModules();
  ({ useToast } = await import('./toast.svelte'));
});

// ── initial state ─────────────────────────────────────────────────────────────

describe('initial state', () => {
  it('starts with no toasts', () => {
    expect(useToast().toasts).toHaveLength(0);
  });
});

// ── addToast() ────────────────────────────────────────────────────────────────

describe('addToast()', () => {
  it('returns a string id', () => {
    const id = useToast().addToast('hello', 'info', 0);
    expect(typeof id).toBe('string');
    expect(id.length).toBeGreaterThan(0);
  });

  it('adds a toast with the given message and type', () => {
    const toast = useToast();
    toast.addToast('Something failed', 'error', 0);
    expect(toast.toasts).toHaveLength(1);
    expect(toast.toasts[0].message).toBe('Something failed');
    expect(toast.toasts[0].type).toBe('error');
  });

  it('defaults to type "info"', () => {
    const toast = useToast();
    toast.addToast('msg');
    expect(toast.toasts[0].type).toBe('info');
  });

  it('defaults to duration 4000', () => {
    const toast = useToast();
    toast.addToast('msg');
    expect(toast.toasts[0].duration).toBe(4000);
  });

  it('assigns each toast a unique id', () => {
    const toast = useToast();
    const id1 = toast.addToast('a', 'info', 0);
    const id2 = toast.addToast('b', 'info', 0);
    expect(id1).not.toBe(id2);
  });

  it('accumulates multiple toasts', () => {
    const toast = useToast();
    toast.addToast('one', 'info', 0);
    toast.addToast('two', 'success', 0);
    toast.addToast('three', 'warning', 0);
    expect(toast.toasts).toHaveLength(3);
  });

  it('caps at 5 toasts and drops the oldest when exceeded', () => {
    const toast = useToast();
    for (let i = 0; i < 6; i++) toast.addToast(`msg ${i}`, 'info', 0);
    expect(toast.toasts).toHaveLength(5);
    expect(toast.toasts[0].message).toBe('msg 1');
    expect(toast.toasts[4].message).toBe('msg 5');
  });

  it('auto-dismisses after the specified duration', () => {
    vi.useFakeTimers();
    const toast = useToast();
    toast.addToast('temporary', 'info', 200);
    expect(toast.toasts).toHaveLength(1);
    vi.advanceTimersByTime(200);
    expect(toast.toasts).toHaveLength(0);
    vi.useRealTimers();
  });

  it('does not auto-dismiss when duration is 0', () => {
    vi.useFakeTimers();
    const toast = useToast();
    toast.addToast('persistent', 'info', 0);
    vi.advanceTimersByTime(60_000);
    expect(toast.toasts).toHaveLength(1);
    vi.useRealTimers();
  });
});

// ── dismiss() ─────────────────────────────────────────────────────────────────

describe('dismiss()', () => {
  it('removes a toast by id', () => {
    const toast = useToast();
    const id = toast.addToast('hello', 'info', 0);
    toast.dismiss(id);
    expect(toast.toasts).toHaveLength(0);
  });

  it('only removes the toast with the matching id', () => {
    const toast = useToast();
    const id1 = toast.addToast('first', 'info', 0);
    toast.addToast('second', 'info', 0);
    toast.dismiss(id1);
    expect(toast.toasts).toHaveLength(1);
    expect(toast.toasts[0].message).toBe('second');
  });

  it('is a no-op for an unknown id', () => {
    const toast = useToast();
    toast.addToast('keep', 'info', 0);
    toast.dismiss('nonexistent-id');
    expect(toast.toasts).toHaveLength(1);
  });

  it('handles dismiss on an already-empty list', () => {
    const toast = useToast();
    expect(() => toast.dismiss('any')).not.toThrow();
    expect(toast.toasts).toHaveLength(0);
  });
});
