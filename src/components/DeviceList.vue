<template>
  <div class="space-y-2">
    <div
      v-for="device in devices"
      :key="device.ip"
      class="flex items-center justify-between p-3 bg-gray-50 rounded-lg hover:bg-gray-100 transition-colors"
    >
      <div class="flex items-center space-x-3">
        <div class="w-3 h-3 bg-green-500 rounded-full"></div>
        <div>
          <div class="font-mono text-sm font-medium text-gray-900">{{ device.ip }}</div>
          <div v-if="device.hostname" class="text-xs text-gray-600">{{ device.hostname }}</div>
          <div v-if="device.mac" class="text-xs text-gray-500 font-mono">{{ device.mac }}</div>
        </div>
      </div>
      <div v-if="device.response_time_ms" class="text-xs text-gray-600">
        {{ device.response_time_ms.toFixed(1) }}ms
      </div>
    </div>
    <div v-if="devices.length === 0" class="text-center text-gray-500 py-8">
      No devices found. Click "Scan Now" to discover devices on your network.
    </div>
  </div>
</template>

<script setup lang="ts">
import type { Device } from '@/stores/agent'

defineProps<{
  devices: Device[]
}>()
</script>

