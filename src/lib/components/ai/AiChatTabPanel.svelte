<!--
  AiChatTabPanel — wraps AiChatThread for use as a standalone center-panel tab
  (the "Open in new tab" action from the AI chat sidebar).
-->
<script lang="ts">
  import AiChatThread from '$lib/components/ai/AiChatThread.svelte';

  interface Props {
    conversationId: string;
  }

  const { conversationId }: Props = $props();
</script>

<div class="ai-chat-tab">
  <AiChatThread {conversationId} showDeleteButton={false} />
</div>

<style>
  .ai-chat-tab {
    display: flex;
    flex-direction: column;
    height: 100%;
    overflow: hidden;
  }
</style>
