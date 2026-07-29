import type { AiChatMode, AiConversation, AiMessage } from '$lib/types';
import type { AiConfig, ChatMessage } from '$lib/ai/service';
import { continueConversation } from '$lib/ai/service';
import * as api from '$lib/tauri/aiChat';

// ── State ─────────────────────────────────────────────────────────────────────

let conversations = $state<AiConversation[]>([]);
let conversationsLoaded = $state(false);
let messagesByConversation = $state<Record<string, AiMessage[]>>({});
// Unsent follow-up drafts, keyed by conversation id — survives navigating away
// from a conversation (sidebar list, another chat, closing a tab) and back.
let draftsByConversation = $state<Record<string, string>>({});

interface CreateConversationInput {
  mode: AiChatMode;
  contextKey: string | null;
  connectionId: string | null;
  database: string | null;
  title: string;
  firstUserMessage: string;
  firstAssistantMessage: string;
}

async function loadMessages(conversationId: string): Promise<AiMessage[]> {
  const msgs = await api.getMessages(conversationId);
  messagesByConversation = { ...messagesByConversation, [conversationId]: msgs };
  return msgs;
}

function upsertConversation(conversation: AiConversation) {
  const idx = conversations.findIndex((c) => c.id === conversation.id);
  if (idx === -1) {
    conversations = [conversation, ...conversations];
  } else {
    conversations = [conversation, ...conversations.slice(0, idx), ...conversations.slice(idx + 1)];
  }
}

export function useAiChat() {
  return {
    get conversations() {
      return conversations;
    },

    get loaded() {
      return conversationsLoaded;
    },

    getMessages(conversationId: string): AiMessage[] {
      return messagesByConversation[conversationId] ?? [];
    },

    getById(id: string): AiConversation | undefined {
      return conversations.find((c) => c.id === id);
    },

    getDraft(conversationId: string): string {
      return draftsByConversation[conversationId] ?? '';
    },

    setDraft(conversationId: string, text: string) {
      if (text) {
        draftsByConversation = { ...draftsByConversation, [conversationId]: text };
      } else if (conversationId in draftsByConversation) {
        const { [conversationId]: _removed, ...rest } = draftsByConversation;
        draftsByConversation = rest;
      }
    },

    async loadConversations() {
      conversations = await api.listConversations();
      conversationsLoaded = true;
    },

    async ensureMessagesLoaded(conversationId: string): Promise<AiMessage[]> {
      const existing = messagesByConversation[conversationId];
      if (existing) return existing;
      return loadMessages(conversationId);
    },

    /** Find the most recently updated conversation for this mode/context/connection/database. */
    async find(
      mode: AiChatMode,
      contextKey: string | null,
      connectionId: string | null,
      database: string | null,
    ): Promise<AiConversation | null> {
      const existing = await api.findConversation(mode, contextKey, connectionId, database);
      if (existing) upsertConversation(existing);
      return existing;
    },

    /** Create a new conversation together with its first user/assistant exchange. */
    async create(
      input: CreateConversationInput,
    ): Promise<{ conversation: AiConversation; messages: AiMessage[] }> {
      const conversation = await api.createConversation({
        mode: input.mode,
        contextKey: input.contextKey,
        title: input.title,
        connectionId: input.connectionId,
        database: input.database,
        firstUserMessage: input.firstUserMessage,
        firstAssistantMessage: input.firstAssistantMessage,
      });
      const messages = await loadMessages(conversation.id);
      upsertConversation(conversation);
      return { conversation, messages };
    },

    /** Send a follow-up in an existing conversation, persist both turns, and return the reply. */
    async sendFollowUp(
      conversationId: string,
      config: AiConfig,
      systemPrompt: string,
      followUpText: string,
      schemaContext?: string,
    ): Promise<string> {
      const priorMessages = messagesByConversation[conversationId] ?? [];
      const history: ChatMessage[] = priorMessages.map((m) => ({
        role: m.role,
        content: m.content,
      }));

      const messageForApi = schemaContext
        ? `Database schema for context:\n${schemaContext}\n\n${followUpText}`
        : followUpText;

      const reply = await continueConversation(config, systemPrompt, history, messageForApi);

      const userMessage = await api.addMessage(conversationId, 'user', followUpText);
      const assistantMessage = await api.addMessage(conversationId, 'assistant', reply);
      messagesByConversation = {
        ...messagesByConversation,
        [conversationId]: [...priorMessages, userMessage, assistantMessage],
      };

      const conversation = conversations.find((c) => c.id === conversationId);
      if (conversation)
        upsertConversation({ ...conversation, updatedAt: assistantMessage.createdAt });

      return reply;
    },

    async rename(id: string, title: string) {
      const updated = await api.renameConversation(id, title);
      upsertConversation(updated);
    },

    async remove(id: string) {
      await api.deleteConversation(id);
      conversations = conversations.filter((c) => c.id !== id);
      const { [id]: _removed, ...rest } = messagesByConversation;
      messagesByConversation = rest;
      if (id in draftsByConversation) {
        const { [id]: _removedDraft, ...restDrafts } = draftsByConversation;
        draftsByConversation = restDrafts;
      }
    },
  };
}
