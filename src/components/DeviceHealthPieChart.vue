<template>
  <div class="flex flex-col items-center justify-center h-full">
    <!-- SVG Donut Chart -->
    <div class="relative">
      <svg :width="size" :height="size" class="transform -rotate-90">
        <!-- Background circle -->
        <circle
          :cx="center"
          :cy="center"
          :r="radius"
          fill="none"
          :stroke-width="strokeWidth"
          class="stroke-dark-700"
        />
        <!-- Healthy segment (green) - starts at 0 -->
        <circle
          v-if="healthyPercent > 0"
          :cx="center"
          :cy="center"
          :r="radius"
          fill="none"
          :stroke-width="strokeWidth"
          class="stroke-green-500"
          :stroke-dasharray="`${healthyDash} ${circumference - healthyDash}`"
          stroke-dashoffset="0"
        />
        <!-- Degraded segment (yellow) - starts after healthy -->
        <circle
          v-if="degradedPercent > 0"
          :cx="center"
          :cy="center"
          :r="radius"
          fill="none"
          :stroke-width="strokeWidth"
          class="stroke-yellow-500"
          :stroke-dasharray="`${degradedDash} ${circumference - degradedDash}`"
          :stroke-dashoffset="-healthyDash"
        />
        <!-- Offline segment (red) - starts after healthy + degraded -->
        <circle
          v-if="offlinePercent > 0"
          :cx="center"
          :cy="center"
          :r="radius"
          fill="none"
          :stroke-width="strokeWidth"
          class="stroke-red-500"
          :stroke-dasharray="`${offlineDash} ${circumference - offlineDash}`"
          :stroke-dashoffset="-(healthyDash + degradedDash)"
        />
      </svg>
      
      <!-- Center text -->
      <div class="absolute inset-0 flex flex-col items-center justify-center">
        <span class="text-3xl font-bold text-white">{{ healthyCount }}/{{ totalDevices }}</span>
        <span class="text-sm text-gray-400">healthy</span>
      </div>
    </div>

    <!-- Legend -->
    <div class="mt-5 space-y-1.5 text-sm">
      <div class="flex items-center gap-2">
        <span class="w-3 h-3 rounded-full bg-green-500"></span>
        <span class="text-gray-300">{{ healthyPercent }}% Healthy ({{ healthyCount }})</span>
      </div>
      <div v-if="degradedCount > 0" class="flex items-center gap-2">
        <span class="w-3 h-3 rounded-full bg-yellow-500"></span>
        <span class="text-gray-300">{{ degradedPercent }}% Degraded ({{ degradedCount }})</span>
      </div>
      <div v-if="offlineCount > 0" class="flex items-center gap-2">
        <span class="w-3 h-3 rounded-full bg-red-500"></span>
        <span class="text-gray-300">{{ offlinePercent }}% Offline ({{ offlineCount }})</span>
      </div>
    </div>

    <!-- View Devices Button -->
    <button
      @click="$emit('view-devices')"
      class="mt-5 text-sm text-brand-cyan hover:text-brand-cyan/80 transition-colors flex items-center gap-1.5"
    >
      {{ totalDevices }} Device{{ totalDevices !== 1 ? 's' : '' }} Found
      <svg class="w-4 h-4" fill="none" viewBox="0 0 24 24" stroke="currentColor">
        <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M9 5l7 7-7 7" />
      </svg>
    </button>
  </div>
</template>

<script setup lang="ts">
import { computed } from 'vue'

interface Props {
  healthyCount: number
  degradedCount: number
  offlineCount: number
  size?: number
  strokeWidth?: number
}

const props = withDefaults(defineProps<Props>(), {
  size: 160,
  strokeWidth: 20
})

defineEmits<{
  'view-devices': []
}>()

const totalDevices = computed(() => props.healthyCount + props.degradedCount + props.offlineCount)

const healthyPercent = computed(() => {
  if (totalDevices.value === 0) return 0
  return Math.round((props.healthyCount / totalDevices.value) * 100)
})

const degradedPercent = computed(() => {
  if (totalDevices.value === 0) return 0
  return Math.round((props.degradedCount / totalDevices.value) * 100)
})

const offlinePercent = computed(() => {
  if (totalDevices.value === 0) return 0
  return Math.round((props.offlineCount / totalDevices.value) * 100)
})

const center = computed(() => props.size / 2)
const radius = computed(() => (props.size - props.strokeWidth) / 2)
const circumference = computed(() => 2 * Math.PI * radius.value)

// Calculate dash length for each segment
const healthyDash = computed(() => {
  if (totalDevices.value === 0) return 0
  return (props.healthyCount / totalDevices.value) * circumference.value
})

const degradedDash = computed(() => {
  if (totalDevices.value === 0) return 0
  return (props.degradedCount / totalDevices.value) * circumference.value
})

const offlineDash = computed(() => {
  if (totalDevices.value === 0) return 0
  return (props.offlineCount / totalDevices.value) * circumference.value
})
</script>
