<template>
  <div class="min-h-screen bg-dark-900">
    <!-- Background gradient effect -->
    <div class="absolute inset-0 bg-gradient-to-br from-brand-cyan/5 via-transparent to-brand-blue/5 pointer-events-none"></div>

    <div class="relative max-w-4xl mx-auto p-6">
      <!-- Header -->
      <div class="bg-dark-800 border border-dark-600 rounded-xl p-6 mb-6">
        <div class="flex items-center justify-between">
          <div>
            <div class="flex items-center gap-3">
              <div class="w-10 h-10 bg-gradient-to-br from-brand-cyan to-brand-blue rounded-lg flex items-center justify-center">
                <svg class="w-6 h-6 text-white" fill="none" viewBox="0 0 24 24" stroke="currentColor">
                  <path stroke-linecap="round" stroke-linejoin="round" stroke-width="1.5" d="M9 20l-5.447-2.724A1 1 0 013 16.382V5.618a1 1 0 011.447-.894L9 7m0 13l6-3m-6 3V7m6 10l4.553 2.276A1 1 0 0021 18.382V7.618a1 1 0 00-.553-.894L15 4m0 13V4m0 0L9 7" />
                </svg>
              </div>
              <div>
                <h1 class="text-xl font-bold text-white">Cartographer Agent</h1>
                <p class="text-sm text-gray-400 flex items-center gap-2">
                  <span class="w-2 h-2 rounded-full" :class="statusDotClass"></span>
                  {{ statusLabel }} as {{ status.userEmail || 'Unknown' }}
                </p>
              </div>
            </div>
            <p v-if="status.networkName" class="text-sm text-brand-cyan mt-2 ml-[52px]">
              {{ status.networkName }}
            </p>
          </div>
          <div class="flex items-center gap-3">
            <button
              @click="handleDisconnect"
              class="text-gray-400 hover:text-red-400 text-sm transition-colors"
              title="Disconnect from cloud"
            >
              Disconnect
            </button>
            <button
              @click="$router.push('/preferences')"
              class="text-gray-400 hover:text-white p-2 rounded-lg hover:bg-dark-700 transition-colors"
              title="Settings"
            >
              <svg class="w-5 h-5" fill="none" viewBox="0 0 24 24" stroke="currentColor">
                <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M10.325 4.317c.426-1.756 2.924-1.756 3.35 0a1.724 1.724 0 002.573 1.066c1.543-.94 3.31.826 2.37 2.37a1.724 1.724 0 001.065 2.572c1.756.426 1.756 2.924 0 3.35a1.724 1.724 0 00-1.066 2.573c.94 1.543-.826 3.31-2.37 2.37a1.724 1.724 0 00-2.572 1.065c-.426 1.756-2.924 1.756-3.35 0a1.724 1.724 0 00-2.573-1.066c-1.543.94-3.31-.826-2.37-2.37a1.724 1.724 0 00-1.065-2.572c-1.756-.426-1.756-2.924 0-3.35a1.724 1.724 0 001.066-2.573c-.94-1.543.826-3.31 2.37-2.37.996.608 2.296.07 2.572-1.065z" />
                <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M15 12a3 3 0 11-6 0 3 3 0 016 0z" />
              </svg>
            </button>
          </div>
        </div>
      </div>

      <!-- Network Info -->
      <div class="bg-dark-800 border border-dark-600 rounded-xl p-6 mb-6">
        <h2 class="text-lg font-semibold text-white mb-4 flex items-center gap-2">
          <svg class="w-5 h-5 text-brand-cyan" fill="none" viewBox="0 0 24 24" stroke="currentColor">
            <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M9 3v2m6-2v2M9 19v2m6-2v2M5 9H3m2 6H3m18-6h-2m2 6h-2M7 19h10a2 2 0 002-2V7a2 2 0 00-2-2H7a2 2 0 00-2 2v10a2 2 0 002 2zM9 9h6v6H9V9z" />
          </svg>
          Network Information
        </h2>
        <div class="grid grid-cols-2 gap-4 text-sm">
          <div>
            <span class="text-gray-400">Network:</span>
            <span class="ml-2 font-mono text-white">{{ networkInfo || 'Detecting...' }}</span>
          </div>
          <div>
            <span class="text-gray-400">Last scan:</span>
            <span class="ml-2 text-white">{{ lastScanTime }}</span>
          </div>
        </div>
      </div>

      <!-- Devices -->
      <div class="bg-dark-800 border border-dark-600 rounded-xl p-6 mb-6">
        <div class="flex items-center justify-between mb-4">
          <h2 class="text-lg font-semibold text-white flex items-center gap-2">
            <svg class="w-5 h-5 text-brand-cyan" fill="none" viewBox="0 0 24 24" stroke="currentColor">
              <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M9.75 17L9 20l-1 1h8l-1-1-.75-3M3 13h18M5 17h14a2 2 0 002-2V5a2 2 0 00-2-2H5a2 2 0 00-2 2v10a2 2 0 002 2z" />
            </svg>
            {{ devices.length }} device{{ devices.length !== 1 ? 's' : '' }} found
          </h2>
          <div class="flex gap-2">
            <button
              @click="handleHealthCheck"
              :disabled="checkingHealth || devices.length === 0"
              class="bg-emerald-600 hover:bg-emerald-500 disabled:bg-dark-600 disabled:text-gray-500 text-white font-medium py-2 px-4 rounded-lg transition-colors flex items-center gap-2"
              title="Check if devices are reachable"
            >
              <svg class="w-4 h-4" fill="none" viewBox="0 0 24 24" stroke="currentColor">
                <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M9 12l2 2 4-4m6 2a9 9 0 11-18 0 9 9 0 0118 0z" />
              </svg>
              {{ checkingHealth ? 'Checking...' : 'Health Check' }}
            </button>
            <button
              @click="handleScan"
              :disabled="scanning"
              class="bg-brand-cyan hover:bg-brand-cyan/90 disabled:bg-dark-600 disabled:text-gray-500 text-dark-900 font-medium py-2 px-4 rounded-lg transition-colors flex items-center gap-2"
            >
              <svg class="w-4 h-4" :class="{ 'animate-spin': scanning }" fill="none" viewBox="0 0 24 24" stroke="currentColor">
                <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M4 4v5h.582m15.356 2A8.001 8.001 0 004.582 9m0 0H9m11 11v-5h-.581m0 0a8.003 8.003 0 01-15.357-2m15.357 2H15" />
              </svg>
              {{ scanning ? 'Scanning...' : 'Scan Now' }}
            </button>
          </div>
        </div>

        <!-- Scan Progress -->
        <div v-if="scanProgress" class="mb-4 p-4 bg-brand-cyan/10 border border-brand-cyan/30 rounded-lg">
          <div class="flex items-center justify-between mb-2">
            <span class="text-sm font-medium text-brand-cyan">
              {{ getScanStageLabel(scanProgress.stage) }}
            </span>
            <span v-if="scanProgress.percent !== null" class="text-xs text-brand-cyan">
              {{ scanProgress.percent }}%
            </span>
          </div>
          <div class="w-full bg-dark-700 rounded-full h-2 mb-2">
            <div
              class="bg-brand-cyan h-2 rounded-full transition-all duration-300"
              :style="{ width: `${scanProgress.percent || 0}%` }"
            ></div>
          </div>
          <p class="text-xs text-gray-400">{{ scanProgress.message }}</p>
          <div class="flex justify-between text-xs text-gray-500 mt-1">
            <span v-if="scanProgress.devicesFound !== null">
              {{ scanProgress.devicesFound }} device{{ scanProgress.devicesFound !== 1 ? 's' : '' }} found
            </span>
            <span>{{ scanProgress.elapsedSecs.toFixed(1) }}s elapsed</span>
          </div>
        </div>

        <!-- Health Check Results -->
        <div v-if="healthStatus && !scanProgress" class="mb-4 p-3 bg-dark-700 rounded-lg text-sm">
          <div class="flex items-center justify-between">
            <div class="flex gap-4">
              <span class="text-green-400 flex items-center gap-1">
                <svg class="w-4 h-4" fill="none" viewBox="0 0 24 24" stroke="currentColor">
                  <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M5 13l4 4L19 7" />
                </svg>
                {{ healthStatus.healthyDevices }} healthy
              </span>
              <span v-if="healthStatus.unreachableDevices > 0" class="text-red-400 flex items-center gap-1">
                <svg class="w-4 h-4" fill="none" viewBox="0 0 24 24" stroke="currentColor">
                  <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M6 18L18 6M6 6l12 12" />
                </svg>
                {{ healthStatus.unreachableDevices }} unreachable
              </span>
            </div>
            <span v-if="healthStatus.syncedToCloud" class="text-brand-cyan text-xs flex items-center gap-1">
              <svg class="w-4 h-4" fill="none" viewBox="0 0 24 24" stroke="currentColor">
                <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M3 15a4 4 0 004 4h9a5 5 0 10-.1-9.999 5.002 5.002 0 10-9.78 2.096A4.001 4.001 0 003 15z" />
              </svg>
              Synced
            </span>
          </div>
        </div>

        <DeviceList :devices="devices" />
      </div>

      <!-- Actions -->
      <div class="flex gap-4">
        <button
          @click="openCloud"
          class="flex-1 bg-dark-800 hover:bg-dark-700 border border-dark-600 text-white font-medium py-3 px-6 rounded-lg transition-colors flex items-center justify-center gap-2"
        >
          View in Cloud
          <svg class="w-4 h-4" fill="none" viewBox="0 0 24 24" stroke="currentColor">
            <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M10 6H6a2 2 0 00-2 2v10a2 2 0 002 2h10a2 2 0 002-2v-4M14 4h6m0 0v6m0-6L10 14" />
          </svg>
        </button>
      </div>
    </div>
  </div>
</template>

<script setup lang="ts">
import { ref, computed, onMounted } from 'vue'
import { useAgentStore, type ScanStage } from '@/stores/agent'
import DeviceList from '@/components/DeviceList.vue'
import { invoke } from '@tauri-apps/api/core'

interface HealthCheckStatus {
  totalDevices: number
  healthyDevices: number
  unreachableDevices: number
  syncedToCloud: boolean
  devices: Array<{
    ip: string
    mac: string | null
    hostname: string | null
    response_time_ms: number | null
  }>
}

const agentStore = useAgentStore()
const networkInfo = ref<string>('')
const checkingHealth = ref(false)
const healthStatus = ref<HealthCheckStatus | null>(null)

const status = computed(() => agentStore.status)
const devices = computed(() => agentStore.devices)
const scanning = computed(() => agentStore.scanning)
const scanProgress = computed(() => agentStore.scanProgress)

// Determine overall network health status for the indicator dot
type NetworkHealthStatus = 'online' | 'degraded' | 'offline'

const networkHealthStatus = computed<NetworkHealthStatus>(() => {
  // If currently scanning or checking health, maintain previous state (default to online)
  if (scanning.value || checkingHealth.value) {
    return 'online'
  }

  // If we have health check results, use them to determine status
  if (healthStatus.value) {
    const { totalDevices, healthyDevices, unreachableDevices } = healthStatus.value

    // All devices unreachable = offline
    if (totalDevices > 0 && unreachableDevices === totalDevices) {
      return 'offline'
    }

    // Some devices unreachable = degraded
    if (unreachableDevices > 0) {
      return 'degraded'
    }

    // All devices healthy = online
    return 'online'
  }

  // No health data yet - default to online (connected state)
  return 'online'
})

const statusDotClass = computed(() => {
  switch (networkHealthStatus.value) {
    case 'online':
      return 'bg-green-500'
    case 'degraded':
      return 'bg-yellow-500'
    case 'offline':
      return 'bg-red-500'
    default:
      return 'bg-green-500'
  }
})

const statusLabel = computed(() => {
  switch (networkHealthStatus.value) {
    case 'online':
      return 'Connected'
    case 'degraded':
      return 'Degraded'
    case 'offline':
      return 'Offline'
    default:
      return 'Connected'
  }
})

// Get human-readable label for scan stage
function getScanStageLabel(stage: ScanStage): string {
  const labels: Record<ScanStage, string> = {
    detecting_network: 'Detecting Network',
    reading_arp: 'Reading Known Devices',
    ping_sweep: 'Discovering Devices',
    resolving_hostnames: 'Resolving Hostnames',
    complete: 'Scan Complete',
    failed: 'Scan Failed'
  }
  return labels[stage] || stage
}

// Computed last scan time that updates when status changes
const lastScanTime = computed(() => {
  if (agentStore.status.lastScan) {
    const date = new Date(agentStore.status.lastScan)
    return date.toLocaleString()
  }
  return 'Never'
})

async function handleScan() {
  try {
    await agentStore.scanNow()
    // Refresh status to get updated lastScan time
    await agentStore.refreshStatus()
    // Clear health status when scanning new devices
    healthStatus.value = null
  } catch (error) {
    console.error('Scan error:', error)
    alert('Failed to scan network. Please try again.')
  }
}

async function handleHealthCheck() {
  checkingHealth.value = true
  try {
    const result = await invoke<HealthCheckStatus>('run_health_check')
    healthStatus.value = result
    // Update devices with latest health data
    if (result.devices && result.devices.length > 0) {
      agentStore.updateDevices(result.devices)
    }
  } catch (error) {
    console.error('Health check error:', error)
    alert(`Health check failed: ${error}`)
  } finally {
    checkingHealth.value = false
  }
}

async function openCloud() {
  try {
    await invoke('open_cloud_dashboard')
  } catch (error) {
    console.error('Failed to open cloud:', error)
  }
}

async function handleDisconnect() {
  if (!confirm('Are you sure you want to disconnect from the cloud? You can reconnect at any time.')) {
    return
  }
  try {
    await agentStore.logout()
    // Navigate back to setup page
    window.location.href = '/'
  } catch (error) {
    console.error('Failed to disconnect:', error)
    alert('Failed to disconnect. Please try again.')
  }
}

async function loadNetworkInfo() {
  try {
    const info = await invoke<string>('get_network_info')
    networkInfo.value = info
  } catch (error) {
    console.error('Failed to get network info:', error)
  }
}

onMounted(async () => {
  await agentStore.refreshStatus()
  await loadNetworkInfo()

  // Refresh status periodically (updates lastScan time automatically)
  setInterval(() => {
    agentStore.refreshStatus()
  }, 30000) // Every 30 seconds
})
</script>
