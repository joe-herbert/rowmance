/**
 * Toast notification store.
 * Provides addToast() for triggering ephemeral notifications,
 * and exposes the reactive toasts array for the Toast component.
 */

export type ToastType = 'info' | 'success' | 'error' | 'warning';

export interface ToastAction {
  label: string;
  onClick: () => void;
}

export interface Toast {
  id: string;
  message: string;
  type: ToastType;
  duration: number;
  action?: ToastAction;
}

const MAX_TOASTS = 5;

let toasts = $state<Toast[]>([]);

let _nextId = 0;
function nextId(): string {
  return String(++_nextId);
}

export function useToast() {
  return {
    get toasts() {
      return toasts;
    },

    addToast(
      message: string,
      type: ToastType = 'info',
      duration = 4000,
      action?: ToastAction,
    ): string {
      const id = nextId();
      const toast: Toast = { id, message, type, duration, action };

      // Cap at MAX_TOASTS — drop the oldest if we're over.
      if (toasts.length >= MAX_TOASTS) {
        toasts = [...toasts.slice(toasts.length - MAX_TOASTS + 1), toast];
      } else {
        toasts = [...toasts, toast];
      }

      if (duration > 0) {
        setTimeout(() => {
          this.dismiss(id);
        }, duration);
      }

      return id;
    },

    dismiss(id: string): void {
      toasts = toasts.filter((t) => t.id !== id);
    },
  };
}
