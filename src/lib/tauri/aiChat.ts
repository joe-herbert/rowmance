/** Typed wrappers around the Tauri `ai_chat::*` commands. */
import { invoke } from '@tauri-apps/api/core';
import type { AiChatMode, AiConversation, AiMessage } from '$lib/types';

export interface AiConversationInput {
  mode: AiChatMode;
  contextKey?: string | null;
  title: string;
  connectionId?: string | null;
  database?: string | null;
  firstUserMessage: string;
  firstAssistantMessage: string;
}

export async function findConversation(
  mode: AiChatMode,
  contextKey: string | null,
  connectionId: string | null,
  database: string | null,
): Promise<AiConversation | null> {
  return invoke<AiConversation | null>('ai_chat_find_conversation', {
    mode,
    contextKey,
    connectionId,
    database,
  });
}

export async function listConversations(): Promise<AiConversation[]> {
  return invoke<AiConversation[]>('ai_chat_list_conversations');
}

export async function createConversation(input: AiConversationInput): Promise<AiConversation> {
  return invoke<AiConversation>('ai_chat_create_conversation', { input });
}

export async function renameConversation(id: string, title: string): Promise<AiConversation> {
  return invoke<AiConversation>('ai_chat_rename_conversation', { id, title });
}

export async function deleteConversation(id: string): Promise<void> {
  return invoke<void>('ai_chat_delete_conversation', { id });
}

export async function getMessages(conversationId: string): Promise<AiMessage[]> {
  return invoke<AiMessage[]>('ai_chat_get_messages', { conversationId });
}

export async function addMessage(
  conversationId: string,
  role: 'user' | 'assistant',
  content: string,
): Promise<AiMessage> {
  return invoke<AiMessage>('ai_chat_add_message', { conversationId, role, content });
}
