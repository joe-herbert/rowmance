import '@testing-library/jest-dom';

// Mock the Tauri invoke API so unit tests never make real IPC calls.
vi.mock('@tauri-apps/api/core', () => ({
  invoke: vi.fn(),
}));
