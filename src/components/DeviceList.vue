<template>
  <div class="space-y-2">
    <div
      v-for="device in devices"
      :key="device.ip"
      class="flex items-center justify-between p-3 bg-dark-700 rounded-lg hover:bg-dark-600 transition-colors border border-dark-600"
    >
      <div class="flex items-center space-x-3">
        <div
          class="w-2.5 h-2.5 rounded-full"
          :class="getDeviceStatusClass(device)"
          :title="getDeviceStatusTitle(device)"
        ></div>
        <div>
          <div class="font-mono text-sm font-medium text-white">{{ device.ip }}</div>
          <div v-if="device.hostname" class="text-xs text-gray-400">{{ device.hostname }}</div>
          <div v-if="device.mac" class="text-xs text-gray-500 font-mono">{{ device.mac }}</div>
        </div>
      </div>
      <div v-if="device.response_time_ms !== null && device.response_time_ms !== undefined" class="text-xs font-mono" :class="device.response_time_ms > 0 ? 'text-brand-cyan' : 'text-yellow-500'">
        {{ device.response_time_ms > 0 ? `${device.response_time_ms.toFixed(1)}ms` : 'ARP' }}
      </div>
    </div>
    <div v-if="devices.length === 0" class="text-center text-gray-500 py-8">
      <svg class="w-12 h-12 mx-auto mb-3 text-gray-600" fill="none" viewBox="0 0 24 24" stroke="currentColor">
        <path stroke-linecap="round" stroke-linejoin="round" stroke-width="1.5" d="M9.75 17L9 20l-1 1h8l-1-1-.75-3M3 13h18M5 17h14a2 2 0 002-2V5a2 2 0 00-2-2H5a2 2 0 00-2 2v10a2 2 0 002 2z" />
      </svg>
      <p>No devices found</p>
      <p class="text-xs text-gray-600 mt-1">Click "Scan Now" to discover devices on your network</p>
    </div>
  </div>
</template>

<script setup lang="ts">
import type { Device } from '@/stores/agent'

defineProps<{
  devices: Device[]
}>()

// Determine device status based on response time
// - Green: Device responded to ping (response_time > 0) OR found in ARP table (response_time = 0)
// - Gray: No response data (device status unknown)
function getDeviceStatusClass(device: Device): string {
  if (device.response_time_ms === null || device.response_time_ms === undefined) {
    return 'bg-gray-500' // Unknown status
  }
  // Both ping responders and ARP-detected devices show as green (online)
  return 'bg-green-500'
}

function getDeviceStatusTitle(device: Device): string {
  if (device.response_time_ms === null || device.response_time_ms === undefined) {
    return 'Status unknown'
  }
  if (device.response_time_ms > 0) {
    return 'Online - responding to ping'
  }
  return 'Online - detected via ARP'
}
</script>
