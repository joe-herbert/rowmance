<script lang="ts">
  import Modal from '$lib/components/Modal.svelte';

  interface Props {
    title: string;
    message: string;
    confirmText?: string;
    cancelText?: string;
    danger?: boolean;
    onconfirm: () => void;
    oncancel: () => void;
  }

  const {
    title,
    message,
    confirmText = 'Confirm',
    cancelText = 'Cancel',
    danger = false,
    onconfirm,
    oncancel,
  }: Props = $props();
</script>

<Modal label={title} onbackdropclick={oncancel}>
  <div class="modal-card">
    <div class="modal-title">{title}</div>
    <div class="modal-body">
      <p class="modal-message">{message}</p>
    </div>
    <div class="modal-footer">
      <button class="btn" onclick={oncancel}>{cancelText}</button>
      <button
        class="btn"
        class:btn--danger={danger}
        class:btn--primary={!danger}
        onclick={onconfirm}
      >
        {confirmText}
      </button>
    </div>
  </div>
</Modal>

<style>
  .modal-card {
    background: var(--color-bg-overlay);
    border: 1px solid var(--color-border-strong);
    border-radius: var(--radius-lg);
    box-shadow: var(--shadow-overlay);
    width: 360px;
    max-width: 92vw;
    overflow: hidden;
    animation: modal-in 140ms ease both;
  }

  @keyframes modal-in {
    from {
      opacity: 0;
      transform: scale(0.96) translateY(-6px);
    }
    to {
      opacity: 1;
      transform: scale(1) translateY(0);
    }
  }

  .modal-title {
    padding: var(--spacing-4) var(--spacing-4) var(--spacing-3);
    font-size: var(--font-size-md);
    font-weight: var(--font-weight-semibold);
    color: var(--color-text-primary);
    border-bottom: 1px solid var(--color-border);
  }

  .modal-body {
    padding: var(--spacing-4);
  }

  .modal-message {
    margin: 0;
    font-size: var(--font-size-sm);
    color: var(--color-text-secondary);
    line-height: var(--line-height-normal);
  }

  .modal-footer {
    padding: var(--spacing-3) var(--spacing-4);
    border-top: 1px solid var(--color-border);
    display: flex;
    align-items: center;
    gap: var(--spacing-2);
    justify-content: flex-end;
  }

  .btn {
    height: 28px;
    padding: 0 14px;
    border-radius: var(--radius-md);
    font-size: var(--font-size-sm);
    font-weight: var(--font-weight-medium);
    cursor: pointer;
    border: 1px solid var(--color-border);
    background: var(--color-bg-secondary);
    color: var(--color-text-secondary);
    transition: all var(--transition-fast);
    white-space: nowrap;
    font-family: var(--font-family-ui);
  }

  .btn:hover {
    border-color: var(--color-border-strong);
    color: var(--color-text-primary);
    background: var(--color-bg-hover);
  }

  .btn--primary {
    background: var(--color-accent);
    border-color: var(--color-accent);
    color: white;
  }

  .btn--primary:hover {
    opacity: 0.88;
    border-color: var(--color-accent);
    background: var(--color-accent);
    color: white;
  }

  .btn--danger {
    background: var(--color-danger, #ef4444);
    border-color: var(--color-danger, #ef4444);
    color: white;
  }

  .btn--danger:hover {
    opacity: 0.88;
    border-color: var(--color-danger, #ef4444);
    background: var(--color-danger, #ef4444);
    color: white;
  }
</style>
