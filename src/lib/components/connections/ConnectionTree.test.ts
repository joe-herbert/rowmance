import { describe, it, expect, vi, beforeEach } from 'vitest';

// Pure functions and logic mirrored from ConnectionTree.svelte for testing
// without rune compilation issues.

// ── Confirm message ───────────────────────────────────────────────────────────

function deleteConfirmMessage(name: string): string {
  return `Delete "${name}"? This cannot be undone.`;
}

// ── Delete orchestration ──────────────────────────────────────────────────────

// Mirrors the deleteConnection() async handler in ConnectionTree.svelte.
async function deleteConnectionHandler(
  profile: { id: string; name: string },
  isActive: (id: string) => boolean,
  confirmFn: (msg: string) => boolean,
  disconnect: (id: string) => Promise<void>,
  deleteProfile: (id: string) => Promise<void>,
  reload: () => Promise<void>,
): Promise<void> {
  if (!confirmFn(deleteConfirmMessage(profile.name))) return;
  if (isActive(profile.id)) {
    await disconnect(profile.id);
  }
  await deleteProfile(profile.id);
  await reload();
}

// ── deleteConfirmMessage ──────────────────────────────────────────────────────

describe('deleteConfirmMessage', () => {
  it('includes the connection name in the message', () => {
    expect(deleteConfirmMessage('My DB')).toContain('My DB');
  });

  it('warns that the action cannot be undone', () => {
    expect(deleteConfirmMessage('x')).toContain('cannot be undone');
  });

  it('wraps the name in quotes', () => {
    expect(deleteConfirmMessage('Prod')).toContain('"Prod"');
  });
});

// ── deleteConnectionHandler ───────────────────────────────────────────────────

describe('deleteConnectionHandler', () => {
  const profile = { id: 'conn-1', name: 'Production DB' };
  let isActive: ReturnType<typeof vi.fn>;
  let confirmFn: ReturnType<typeof vi.fn>;
  let disconnect: ReturnType<typeof vi.fn>;
  let deleteProfile: ReturnType<typeof vi.fn>;
  let reload: ReturnType<typeof vi.fn>;

  beforeEach(() => {
    isActive = vi.fn().mockReturnValue(false);
    confirmFn = vi.fn().mockReturnValue(true);
    disconnect = vi.fn().mockResolvedValue(undefined);
    deleteProfile = vi.fn().mockResolvedValue(undefined);
    reload = vi.fn().mockResolvedValue(undefined);
  });

  it('calls deleteProfile and reload when confirmed and not active', async () => {
    await deleteConnectionHandler(profile, isActive, confirmFn, disconnect, deleteProfile, reload);

    expect(deleteProfile).toHaveBeenCalledWith('conn-1');
    expect(reload).toHaveBeenCalledOnce();
  });

  it('does not call deleteProfile when user cancels the confirm dialog', async () => {
    confirmFn.mockReturnValue(false);

    await deleteConnectionHandler(profile, isActive, confirmFn, disconnect, deleteProfile, reload);

    expect(deleteProfile).not.toHaveBeenCalled();
    expect(reload).not.toHaveBeenCalled();
  });

  it('disconnects before deleting when the connection is active', async () => {
    isActive.mockReturnValue(true);
    const callOrder: string[] = [];
    disconnect.mockImplementation(async () => { callOrder.push('disconnect'); });
    deleteProfile.mockImplementation(async () => { callOrder.push('delete'); });

    await deleteConnectionHandler(profile, isActive, confirmFn, disconnect, deleteProfile, reload);

    expect(disconnect).toHaveBeenCalledWith('conn-1');
    expect(deleteProfile).toHaveBeenCalledWith('conn-1');
    expect(callOrder).toEqual(['disconnect', 'delete']);
  });

  it('does not call disconnect when the connection is not active', async () => {
    isActive.mockReturnValue(false);

    await deleteConnectionHandler(profile, isActive, confirmFn, disconnect, deleteProfile, reload);

    expect(disconnect).not.toHaveBeenCalled();
  });

  it('passes the connection name to the confirm message', async () => {
    await deleteConnectionHandler(profile, isActive, confirmFn, disconnect, deleteProfile, reload);

    expect(confirmFn).toHaveBeenCalledWith(expect.stringContaining('Production DB'));
  });

  it('calls reload after a successful delete', async () => {
    const callOrder: string[] = [];
    deleteProfile.mockImplementation(async () => { callOrder.push('delete'); });
    reload.mockImplementation(async () => { callOrder.push('reload'); });

    await deleteConnectionHandler(profile, isActive, confirmFn, disconnect, deleteProfile, reload);

    expect(callOrder).toEqual(['delete', 'reload']);
  });
});
