<template>
  <Transition name="slide-up">
    <div
      v-if="show"
      class="fixed bottom-0 left-0 right-0 bg-blue-600 dark:bg-blue-700 text-white px-4 py-3 flex items-center justify-between z-50 shadow-lg"
    >
      <div class="flex items-center gap-3">
        <svg
          class="w-5 h-5 flex-shrink-0"
          fill="none"
          stroke="currentColor"
          viewBox="0 0 24 24"
        >
          <path
            stroke-linecap="round"
            stroke-linejoin="round"
            stroke-width="2"
            d="M9 12l2 2 4-4m6 2a9 9 0 11-18 0 9 9 0 0118 0z"
          />
        </svg>
        <span class="text-sm font-medium">
          Cartographer Agent updated to version {{ version }}
        </span>
      </div>
      <button
        @click="dismiss"
        class="p-1 hover:bg-blue-700 dark:hover:bg-blue-800 rounded transition-colors"
        title="Dismiss"
      >
        <svg
          class="w-5 h-5"
          fill="none"
          stroke="currentColor"
          viewBox="0 0 24 24"
        >
          <path
            stroke-linecap="round"
            stroke-linejoin="round"
            stroke-width="2"
            d="M6 18L18 6M6 6l12 12"
          />
        </svg>
      </button>
    </div>
  </Transition>
</template>

<script setup lang="ts">
import { ref, onMounted, onUnmounted, watch } from 'vue'
import { listen } from '@tauri-apps/api/event'
import type { UnlistenFn } from '@tauri-apps/api/event'

interface SilentUpdateCompletedEvent {
  version: string
}

const show = ref(false)
const version = ref('')
let unlisten: UnlistenFn | null = null
let dismissTimer: ReturnType<typeof setTimeout> | null = null

function dismiss() {
  show.value = false
  if (dismissTimer) {
    clearTimeout(dismissTimer)
    dismissTimer = null
  }
}

function startDismissTimer() {
  if (dismissTimer) {
    clearTimeout(dismissTimer)
  }
  dismissTimer = setTimeout(() => {
    show.value = false
  }, 5000)
}

onMounted(async () => {
  unlisten = await listen<SilentUpdateCompletedEvent>('silent-update-completed', (event) => {
    console.log('Silent update completed event received:', event.payload)
    version.value = event.payload.version
    show.value = true
    startDismissTimer()
  })
})

onUnmounted(() => {
  if (unlisten) {
    unlisten()
  }
  if (dismissTimer) {
    clearTimeout(dismissTimer)
  }
})
</script>

<style scoped>
.slide-up-enter-active,
.slide-up-leave-active {
  transition: transform 0.3s ease, opacity 0.3s ease;
}

.slide-up-enter-from,
.slide-up-leave-to {
  transform: translateY(100%);
  opacity: 0;
}
</style>
