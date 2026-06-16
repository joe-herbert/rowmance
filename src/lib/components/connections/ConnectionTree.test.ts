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

// ── statusClass / statusLabel ─────────────────────────────────────────────────

// Mirrors the statusClass and statusLabel helpers in ConnectionTree.svelte.
function statusClass(
  profileId: string,
  connectingIds: Set<string>,
  errorIds: Map<string, string>,
  activeIds: Set<string>,
): string {
  if (connectingIds.has(profileId)) return 'connecting';
  if (errorIds.has(profileId)) return 'error';
  if (activeIds.has(profileId)) return 'connected';
  return 'idle';
}

function statusLabel(
  profileId: string,
  connectingIds: Set<string>,
  errorIds: Map<string, string>,
  activeIds: Set<string>,
): string {
  const status = statusClass(profileId, connectingIds, errorIds, activeIds);
  const labels: Record<string, string> = {
    connected: 'Connected',
    connecting: 'Connecting…',
    error: errorIds.get(profileId) ?? 'Connection error',
    idle: 'Not connected',
  };
  return labels[status];
}

describe('statusClass', () => {
  const id = 'conn-1';

  it('returns "connecting" when the id is in connectingIds', () => {
    expect(statusClass(id, new Set([id]), new Map(), new Set())).toBe('connecting');
  });

  it('returns "error" when the id is in errorIds (and not connecting)', () => {
    expect(statusClass(id, new Set(), new Map([[id, 'timeout']]), new Set())).toBe('error');
  });

  it('returns "connected" when the id is in activeIds', () => {
    expect(statusClass(id, new Set(), new Map(), new Set([id]))).toBe('connected');
  });

  it('returns "idle" when the id appears in none of the sets', () => {
    expect(statusClass(id, new Set(), new Map(), new Set())).toBe('idle');
  });

  it('connecting takes priority over error', () => {
    expect(
      statusClass(id, new Set([id]), new Map([[id, 'err']]), new Set()),
    ).toBe('connecting');
  });
});

describe('statusLabel', () => {
  const id = 'conn-1';

  it('returns "Connected" for an active connection', () => {
    expect(statusLabel(id, new Set(), new Map(), new Set([id]))).toBe('Connected');
  });

  it('returns "Connecting…" while connecting', () => {
    expect(statusLabel(id, new Set([id]), new Map(), new Set())).toBe('Connecting…');
  });

  it('returns the error message for a failed connection', () => {
    expect(
      statusLabel(id, new Set(), new Map([[id, 'Connection refused']]), new Set()),
    ).toBe('Connection refused');
  });

  it('returns the stored error message verbatim', () => {
    expect(
      statusLabel(id, new Set(), new Map([[id, 'Connection refused']]), new Set()),
    ).toBe('Connection refused');
  });

  it('returns "Not connected" for an idle connection', () => {
    expect(statusLabel(id, new Set(), new Map(), new Set())).toBe('Not connected');
  });
});

// ── retry handler ─────────────────────────────────────────────────────────────

// Mirrors handleConnect() in ConnectionTree — retry simply calls connect() again,
// which internally clears the error for that id before re-attempting.
async function retryHandler(
  profileId: string,
  isActive: (id: string) => boolean,
  connect: (id: string) => Promise<void>,
): Promise<void> {
  if (isActive(profileId)) return;
  await connect(profileId);
}

describe('retryHandler', () => {
  it('calls connect with the profile id', async () => {
    const connect = vi.fn().mockResolvedValue(undefined);
    await retryHandler('conn-1', () => false, connect);
    expect(connect).toHaveBeenCalledWith('conn-1');
  });

  it('does not call connect when the connection is already active', async () => {
    const connect = vi.fn().mockResolvedValue(undefined);
    await retryHandler('conn-1', () => true, connect);
    expect(connect).not.toHaveBeenCalled();
  });

  it('propagates errors thrown by connect', async () => {
    const connect = vi.fn().mockRejectedValue(new Error('still failing'));
    await expect(retryHandler('conn-1', () => false, connect)).rejects.toThrow('still failing');
  });
});
