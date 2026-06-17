import { describe, it, expect, vi, beforeEach } from 'vitest';

vi.mock('@tauri-apps/api/webviewWindow', () => ({
  WebviewWindow: vi.fn(),
}));

import { WebviewWindow } from '@tauri-apps/api/webviewWindow';
import { openNewWindow } from './window';

const MockWebviewWindow = vi.mocked(WebviewWindow);

beforeEach(() => {
  MockWebviewWindow.mockReset();
});

describe('openNewWindow', () => {
  it('constructs a WebviewWindow with url "/"', () => {
    openNewWindow();
    expect(MockWebviewWindow).toHaveBeenCalledOnce();
    const [, options] = MockWebviewWindow.mock.calls[0];
    expect(options!.url).toBe('/');
  });

  it('sets title to "rowmance"', () => {
    openNewWindow();
    const [, options] = MockWebviewWindow.mock.calls[0];
    expect(options!.title).toBe('rowmance');
  });

  it('uses a unique label on every call', () => {
    openNewWindow();
    openNewWindow();
    const label1 = MockWebviewWindow.mock.calls[0][0] as string;
    const label2 = MockWebviewWindow.mock.calls[1][0] as string;
    expect(label1).not.toBe(label2);
    expect(label1).toMatch(/^rowmance-\d+$/);
    expect(label2).toMatch(/^rowmance-\d+$/);
  });

  it('sets default dimensions', () => {
    openNewWindow();
    const [, options] = MockWebviewWindow.mock.calls[0];
    expect(options!.width).toBe(800);
    expect(options!.height).toBe(600);
  });
});
