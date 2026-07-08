/**
 * Global state for tab dragging across split boundaries.
 * When a tab drag starts in one split and the pointer moves toward
 * an edge zone or another split's tab bar, this store communicates
 * the dragged item identity and source split to drop targets.
 *
 * Also supports content drags (e.g. dragging a table name from the schema tree)
 * where there is no existing open item — only content to open in the target split.
 */
import type { PanelKind } from '$lib/types';

let draggingItemId = $state<string | null>(null);
let draggingSourceSplitId = $state<string | null>(null);
let draggingContent = $state<PanelKind | null>(null);

export function useTabDrag() {
  return {
    get isDragging() {
      return draggingItemId !== null || draggingContent !== null;
    },
    get itemId() {
      return draggingItemId;
    },
    get sourceSplitId() {
      return draggingSourceSplitId;
    },
    get dragContent() {
      return draggingContent;
    },
    start(itemId: string, splitId: string) {
      draggingItemId = itemId;
      draggingSourceSplitId = splitId;
      draggingContent = null;
      document.body.classList.add('tab-dragging');
    },
    startContent(content: PanelKind) {
      draggingContent = content;
      draggingItemId = null;
      draggingSourceSplitId = null;
      document.body.classList.add('tab-dragging');
    },
    end() {
      draggingItemId = null;
      draggingSourceSplitId = null;
      draggingContent = null;
      document.body.classList.remove('tab-dragging');
    },
  };
}
