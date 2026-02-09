<script lang="ts">
  import { onMount } from 'svelte';
  import { chatsStore } from '$lib/stores/chats.svelte';
  import { settingsStore } from '$lib/stores/settings.svelte';
  import { ragStore } from '$lib/stores/rag.svelte';
  import { blenderStore } from '$lib/stores/blender.svelte';
  import { askQuestion } from '$lib/utils/api';

  import Sidebar from '$lib/components/Sidebar.svelte';
  import ChatMessage from '$lib/components/ChatMessage.svelte';
  import ChatInput from '$lib/components/ChatInput.svelte';
  import StatusIndicator from '$lib/components/StatusIndicator.svelte';
  import BlenderIndicator from '$lib/components/BlenderIndicator.svelte';
  import ScenePanel from '$lib/components/ScenePanel.svelte';
  import SuggestionList from '$lib/components/SuggestionList.svelte';

  import { Menu, Moon, Sun, MessageSquare, Sparkles } from 'lucide-svelte';

  type Tab = 'chat' | 'suggestions';

  let currentTab = $state<Tab>('chat');
  let isSidebarOpen = $state(true);
  let isWaitingForResponse = $state(false);
  let serverReady = $state(false);
  let chatContainer = $state<HTMLDivElement>();
  let stopRagPolling: (() => void) | null = null;
  let stopBlenderPolling: (() => void) | null = null;

  const currentChat = $derived(chatsStore.currentChat);
  const messages = $derived(currentChat?.messages ?? []);

  // Auto-scroll logic
  let shouldAutoScroll = $state(true);

  function checkScrollPosition() {
    if (!chatContainer) return;
    const { scrollTop, scrollHeight, clientHeight } = chatContainer;
    const distanceFromBottom = scrollHeight - scrollTop - clientHeight;
    shouldAutoScroll = distanceFromBottom < 100;
  }

  function scrollToBottom() {
    if (chatContainer && shouldAutoScroll && settingsStore.autoScrollChat) {
      setTimeout(() => {
        if (chatContainer) {
          chatContainer.scrollTop = chatContainer.scrollHeight;
        }
      }, 0);
    }
  }

  $effect(() => {
    // Auto-scroll when messages change
    if (messages) {
      scrollToBottom();
    }
  });

  // Initialize chat if none exists
  $effect(() => {
    if (!currentChat && chatsStore.chats.length === 0) {
      chatsStore.createChat();
    } else if (!currentChat && chatsStore.chats.length > 0) {
      chatsStore.setCurrentChat(chatsStore.chats[0].id);
    }
  });

  async function handleSendMessage(content: string) {
    if (!currentChat || isWaitingForResponse) return;

    const chatId = currentChat.id;

    // Add user message
    const userMessage = {
      id: crypto.randomUUID(),
      role: 'user' as const,
      content,
      timestamp: Date.now()
    };
    chatsStore.addMessage(chatId, userMessage);

    // Add assistant message placeholder
    const assistantMessageId = crypto.randomUUID();
    const assistantMessage = {
      id: assistantMessageId,
      role: 'assistant' as const,
      content: '',
      timestamp: Date.now(),
      isStreaming: true
    };
    chatsStore.addMessage(chatId, assistantMessage);

    isWaitingForResponse = true;

    try {
      // Get scene context if available
      const scene_context = blenderStore.getSceneContext();

      // Call RAG server
      const response = await askQuestion({
        question: content,
        scene_context: scene_context ?? undefined
      });

      // Update assistant message with response
      chatsStore.updateMessage(chatId, assistantMessageId, {
        content: response.answer,
        isStreaming: false
      });
    } catch (error) {
      const errorMessage = error instanceof Error ? error.message : 'Failed to get response';
      chatsStore.updateMessage(chatId, assistantMessageId, {
        content: `Error: ${errorMessage}`,
        isStreaming: false
      });
    } finally {
      isWaitingForResponse = false;
    }
  }

  function handleExampleSelect(example: string) {
    handleSendMessage(example);
  }

  function toggleSidebar() {
    isSidebarOpen = !isSidebarOpen;
  }

  function toggleTheme() {
    const newTheme = settingsStore.theme === 'dark' ? 'light' : 'dark';
    settingsStore.setTheme(newTheme);
  }

  onMount(async () => {
    // Wait for server to be ready before showing UI
    let retries = 0;
    const maxRetries = 30; // 30 seconds

    while (retries < maxRetries && !ragStore.isConnected) {
      await ragStore.checkStatus();
      if (ragStore.isConnected) {
        serverReady = true;
        break;
      }
      await new Promise(resolve => setTimeout(resolve, 1000));
      retries++;
    }

    if (!ragStore.isConnected) {
      // Show warning but still allow app to load
      console.warn('Server did not respond within 30 seconds');
      serverReady = true; // Still show the UI
    }

    // Start RAG server polling
    stopRagPolling = ragStore.startPolling(settingsStore.pollingInterval);

    // Start Blender scene polling
    stopBlenderPolling = blenderStore.startPolling(5000);

    // Cleanup on unmount
    return () => {
      if (stopRagPolling) {
        stopRagPolling();
      }
      if (stopBlenderPolling) {
        stopBlenderPolling();
      }
    };
  });
</script>

{#if !serverReady}
  <!-- Loading Screen -->
  <div class="flex h-screen w-screen items-center justify-center bg-[var(--background)]">
    <div class="text-center">
      <!-- Spinner -->
      <div class="mb-6 inline-block h-12 w-12 animate-spin rounded-full border-4 border-solid border-[var(--primary)] border-r-transparent"></div>

      <h2 class="text-2xl font-semibold mb-2 text-[var(--foreground)]">
        Starting Blender Learning Assistant...
      </h2>
      <p class="text-sm text-[var(--muted-foreground)]">
        Initializing AI server
      </p>
    </div>
  </div>
{:else}
<div class="flex h-screen overflow-hidden">
  <!-- Sidebar -->
  <Sidebar isOpen={isSidebarOpen} onClose={toggleSidebar} />

  <!-- Main Content -->
  <div class="flex-1 flex flex-col min-w-0">
    <!-- Header -->
    <header class="border-b border-[var(--border)] bg-[var(--card)] px-4 py-3 shadow-sm">
      <div class="flex items-center justify-between">
        <div class="flex items-center gap-3">
          <button
            onclick={toggleSidebar}
            class="p-2 hover:bg-[var(--accent)] rounded-lg transition-all hover:scale-105 active:scale-95"
            aria-label="Toggle sidebar"
          >
            <Menu size={20} class="transition-transform" />
          </button>

          <h1 class="text-lg font-semibold text-[var(--foreground)]">
            Blender Learning Assistant
          </h1>
        </div>

        <div class="flex items-center gap-2">
          <StatusIndicator />
          <BlenderIndicator />

          <button
            onclick={toggleTheme}
            class="p-2 hover:bg-[var(--accent)] rounded-lg transition-all hover:scale-105 active:scale-95"
            aria-label="Toggle theme"
          >
            {#if settingsStore.theme === 'dark'}
              <Sun size={20} class="transition-transform rotate-0 hover:rotate-12" />
            {:else}
              <Moon size={20} class="transition-transform rotate-0 hover:-rotate-12" />
            {/if}
          </button>
        </div>
      </div>
    </header>

    <!-- Tabs -->
    <div class="border-b border-[var(--border)] bg-[var(--card)] px-4">
      <div class="flex gap-1">
        <button
          onclick={() => { currentTab = 'chat'; }}
          class="px-4 py-3 font-medium text-sm transition-all relative rounded-t-lg
                 {currentTab === 'chat'
                   ? 'text-[var(--primary)] bg-[var(--primary)]/5'
                   : 'text-[var(--muted-foreground)] hover:text-[var(--foreground)] hover:bg-[var(--accent)]'}"
        >
          <div class="flex items-center gap-2">
            <MessageSquare size={16} class="transition-transform {currentTab === 'chat' ? 'scale-110' : ''}" />
            Chat
          </div>
          {#if currentTab === 'chat'}
            <div class="absolute bottom-0 left-0 right-0 h-0.5 bg-[var(--primary)] rounded-full"></div>
          {/if}
        </button>

        <button
          onclick={() => { currentTab = 'suggestions'; }}
          class="px-4 py-3 font-medium text-sm transition-all relative rounded-t-lg
                 {currentTab === 'suggestions'
                   ? 'text-[var(--primary)] bg-[var(--primary)]/5'
                   : 'text-[var(--muted-foreground)] hover:text-[var(--foreground)] hover:bg-[var(--accent)]'}"
        >
          <div class="flex items-center gap-2">
            <Sparkles size={16} class="transition-transform {currentTab === 'suggestions' ? 'scale-110' : ''}" />
            Suggestions
          </div>
          {#if currentTab === 'suggestions'}
            <div class="absolute bottom-0 left-0 right-0 h-0.5 bg-[var(--primary)] rounded-full"></div>
          {/if}
        </button>
      </div>
    </div>

    <!-- Tab Content -->
    <div class="flex-1 overflow-hidden flex flex-col">
      {#if currentTab === 'chat'}
        <!-- Chat View -->
        <div
          bind:this={chatContainer}
          onscroll={checkScrollPosition}
          class="flex-1 overflow-y-auto scrollbar-thin"
        >
          {#if messages.length === 0}
            <!-- Empty State - ChatGPT Style -->
            <div class="flex flex-col items-center justify-center h-full px-6">
              <div class="w-full max-w-3xl mx-auto text-center mb-8">
                <h1 class="text-3xl md:text-4xl font-medium text-[var(--foreground)] mb-10">
                  What can I help you create in Blender today?
                </h1>

                <!-- Quick Examples Grid -->
                <div class="grid grid-cols-1 md:grid-cols-2 gap-3 max-w-2xl mx-auto">
                  <button
                    onclick={() => handleExampleSelect('How do I create a UV map for my model?')}
                    class="p-4 rounded-lg border border-[var(--border)] hover:bg-[var(--accent)] transition-all text-left hover:border-[var(--primary)]"
                  >
                    <div class="text-sm font-medium mb-1 text-[var(--foreground)]">UV Mapping</div>
                    <div class="text-xs text-[var(--muted-foreground)]">Create a UV map for my model</div>
                  </button>

                  <button
                    onclick={() => handleExampleSelect('What are the different types of modifiers in Blender?')}
                    class="p-4 rounded-lg border border-[var(--border)] hover:bg-[var(--accent)] transition-all text-left hover:border-[var(--primary)]"
                  >
                    <div class="text-sm font-medium mb-1 text-[var(--foreground)]">Modifiers Guide</div>
                    <div class="text-xs text-[var(--muted-foreground)]">Explain different modifier types</div>
                  </button>

                  <button
                    onclick={() => handleExampleSelect('How can I improve my render times?')}
                    class="p-4 rounded-lg border border-[var(--border)] hover:bg-[var(--accent)] transition-all text-left hover:border-[var(--primary)]"
                  >
                    <div class="text-sm font-medium mb-1 text-[var(--foreground)]">Rendering Optimization</div>
                    <div class="text-xs text-[var(--muted-foreground)]">Speed up my render times</div>
                  </button>

                  <button
                    onclick={() => handleExampleSelect('What is the best way to model hard surface objects?')}
                    class="p-4 rounded-lg border border-[var(--border)] hover:bg-[var(--accent)] transition-all text-left hover:border-[var(--primary)]"
                  >
                    <div class="text-sm font-medium mb-1 text-[var(--foreground)]">Hard Surface Modeling</div>
                    <div class="text-xs text-[var(--muted-foreground)]">Best practices for hard surfaces</div>
                  </button>
                </div>
              </div>
            </div>
          {:else}
            <!-- Messages -->
            <div class="max-w-4xl mx-auto w-full">
              {#if settingsStore.showScenePanel}
                <div class="px-6 pt-4 pb-2">
                  <ScenePanel />
                </div>
              {/if}

              {#each messages as message (message.id)}
                <ChatMessage {message} />
              {/each}
            </div>
          {/if}
        </div>

        <!-- Chat Input -->
        <ChatInput
          onSubmit={handleSendMessage}
          disabled={isWaitingForResponse || !ragStore.isConnected}
          placeholder={ragStore.isConnected
            ? 'Ask about Blender...'
            : 'Waiting for RAG server connection...'}
        />
      {:else if currentTab === 'suggestions'}
        <!-- Suggestions View -->
        <div class="flex-1 overflow-y-auto scrollbar-thin p-4">
          <div class="max-w-4xl mx-auto">
            {#if settingsStore.showScenePanel}
              <div class="mb-4">
                <ScenePanel />
              </div>
            {/if}

            <SuggestionList />
          </div>
        </div>
      {/if}
    </div>
  </div>
</div>
{/if}
