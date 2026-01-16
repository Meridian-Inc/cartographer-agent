<template>
  <div
    class="h-8 bg-dark-800 border-b border-dark-600 flex items-center justify-between px-3 select-none"
    data-tauri-drag-region
  >
    <!-- Left side: App icon and title -->
    <div class="flex items-center gap-2" data-tauri-drag-region>
      <div class="w-4 h-4 bg-gradient-to-br from-brand-cyan to-brand-blue rounded flex items-center justify-center">
        <svg class="w-2.5 h-2.5 text-white" fill="none" viewBox="0 0 24 24" stroke="currentColor">
          <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M9 20l-5.447-2.724A1 1 0 013 16.382V5.618a1 1 0 011.447-.894L9 7m0 13l6-3m-6 3V7m6 10l4.553 2.276A1 1 0 0021 18.382V7.618a1 1 0 00-.553-.894L15 4m0 13V4m0 0L9 7" />
        </svg>
      </div>
      <span class="text-xs text-gray-400 font-medium" data-tauri-drag-region>Cartographer Agent</span>
    </div>

    <!-- Right side: Window controls -->
    <div class="flex items-center gap-1">
      <!-- Minimize -->
      <button
        @click="minimize"
        class="w-6 h-6 flex items-center justify-center text-gray-400 hover:text-white hover:bg-dark-600 rounded transition-colors"
        title="Minimize"
      >
        <svg class="w-3 h-3" fill="none" viewBox="0 0 24 24" stroke="currentColor">
          <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M20 12H4" />
        </svg>
      </button>

      <!-- Maximize/Restore -->
      <button
        @click="toggleMaximize"
        class="w-6 h-6 flex items-center justify-center text-gray-400 hover:text-white hover:bg-dark-600 rounded transition-colors"
        :title="isMaximized ? 'Restore' : 'Maximize'"
      >
        <svg v-if="!isMaximized" class="w-3 h-3" fill="none" viewBox="0 0 24 24" stroke="currentColor">
          <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M4 8V4h16v16h-4M4 8h12v12H4V8z" />
        </svg>
        <svg v-else class="w-3 h-3" fill="none" viewBox="0 0 24 24" stroke="currentColor">
          <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M8 4h12v12M4 8h12v12H4z" />
        </svg>
      </button>

      <!-- Close -->
      <button
        @click="close"
        class="w-6 h-6 flex items-center justify-center text-gray-400 hover:text-white hover:bg-red-600 rounded transition-colors"
        title="Close"
      >
        <svg class="w-3 h-3" fill="none" viewBox="0 0 24 24" stroke="currentColor">
          <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M6 18L18 6M6 6l12 12" />
        </svg>
      </button>
    </div>
  </div>
</template>

<script setup lang="ts">
import { ref, onMounted, onUnmounted } from 'vue'
import { getCurrentWindow } from '@tauri-apps/api/window'

const isMaximized = ref(false)
let unlisten: (() => void) | null = null

async function minimize() {
  const appWindow = getCurrentWindow()
  await appWindow.minimize()
}

async function toggleMaximize() {
  const appWindow = getCurrentWindow()
  if (isMaximized.value) {
    await appWindow.unmaximize()
  } else {
    await appWindow.maximize()
  }
}

async function close() {
  const appWindow = getCurrentWindow()
  await appWindow.hide()
}

async function updateMaximizedState() {
  const appWindow = getCurrentWindow()
  isMaximized.value = await appWindow.isMaximized()
}

onMounted(async () => {
  await updateMaximizedState()
  
  // Listen for window resize events to update maximize state
  const appWindow = getCurrentWindow()
  unlisten = await appWindow.onResized(async () => {
    await updateMaximizedState()
  })
})

onUnmounted(() => {
  if (unlisten) {
    unlisten()
  }
})
</script>
