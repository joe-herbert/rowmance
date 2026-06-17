export function portal(node: HTMLElement, target: HTMLElement | string = 'body') {
  let targetEl: HTMLElement;

  function mount(t: HTMLElement | string) {
    targetEl = typeof t === 'string' ? (document.querySelector(t) as HTMLElement) : t;
    targetEl.appendChild(node);
  }

  function destroy() {
    node.parentNode?.removeChild(node);
  }

  mount(target);
  return { update: mount, destroy };
}
