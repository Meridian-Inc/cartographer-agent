<template>
  <div class="flex items-center space-x-2 text-sm">
    <div
      :class="[
        'w-2 h-2 rounded-full',
        statusClass
      ]"
    ></div>
    <span class="text-gray-600">{{ statusText }}</span>
  </div>
</template>

<script setup lang="ts">
import { computed } from 'vue'

const props = defineProps<{
  status: 'connected' | 'scanning' | 'disconnected' | 'error'
}>()

const statusClass = computed(() => {
  switch (props.status) {
    case 'connected':
      return 'bg-green-500'
    case 'scanning':
      return 'bg-yellow-500'
    case 'disconnected':
      return 'bg-red-500'
    case 'error':
      return 'bg-red-500'
    default:
      return 'bg-gray-400'
  }
})

const statusText = computed(() => {
  switch (props.status) {
    case 'connected':
      return 'Connected'
    case 'scanning':
      return 'Scanning...'
    case 'disconnected':
      return 'Disconnected'
    case 'error':
      return 'Error'
    default:
      return 'Unknown'
  }
})
</script>

