/**
 * Tags store.
 * Manages the global list of tags that can be attached to connections.
 */
import type { Tag, TagInput } from '$lib/types';
import * as api from '$lib/tauri/tags';

// ── State ─────────────────────────────────────────────────────────────────────

let tags = $state<Tag[]>([]);

// ── Public interface ──────────────────────────────────────────────────────────

export function useTags() {
  return {
    get tags() {
      return tags;
    },

    /** Load all tags from the backend. */
    async load(): Promise<void> {
      tags = await api.listTags();
    },

    /** Create a new tag. */
    async create(input: TagInput): Promise<Tag> {
      const created = await api.createTag(input);
      tags = [...tags, created];
      return created;
    },

    /** Update an existing tag's name and/or color. */
    async update(id: string, input: TagInput): Promise<Tag> {
      const updated = await api.updateTag(id, input);
      tags = tags.map((t) => (t.id === id ? updated : t));
      return updated;
    },

    /** Delete a tag. */
    async remove(id: string): Promise<void> {
      await api.deleteTag(id);
      tags = tags.filter((t) => t.id !== id);
    },

    /** Reorder tags. `ids` is the full list of tag ids in their new display order. */
    async reorder(ids: string[]): Promise<void> {
      const byId = new Map(tags.map((t) => [t.id, t]));
      tags = ids.map((id) => byId.get(id)).filter((t) => t !== undefined);
      await api.reorderTags(ids);
    },

    /** Return the tag with the given ID, or undefined. */
    getById(id: string): Tag | undefined {
      return tags.find((t) => t.id === id);
    },
  };
}
