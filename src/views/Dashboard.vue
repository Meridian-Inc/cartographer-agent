<template>
  <div class="min-h-screen bg-gray-50">
    <div class="max-w-4xl mx-auto p-6">
      <!-- Header -->
      <div class="bg-white rounded-lg shadow-sm p-6 mb-6">
        <div class="flex items-center justify-between">
          <div>
            <h1 class="text-2xl font-bold text-gray-900">Cartographer Agent</h1>
            <p class="text-sm text-gray-600 mt-1">
              <span class="inline-block w-2 h-2 bg-green-500 rounded-full mr-2"></span>
              Connected as {{ status.userEmail || 'Unknown' }}
            </p>
            <p v-if="status.networkName" class="text-sm text-indigo-600 mt-1">
              📡 {{ status.networkName }}
            </p>
          </div>
          <div class="flex items-center gap-2">
            <button
              @click="handleDisconnect"
              class="text-gray-500 hover:text-red-600 text-sm"
              title="Disconnect from cloud"
            >
              Disconnect
            </button>
            <button
              @click="$router.push('/preferences')"
              class="text-gray-600 hover:text-gray-900"
            >
              ⚙️ Settings
            </button>
          </div>
        </div>
      </div>

      <!-- Network Info -->
      <div class="bg-white rounded-lg shadow-sm p-6 mb-6">
        <h2 class="text-lg font-semibold text-gray-900 mb-4">Network Information</h2>
        <div class="grid grid-cols-2 gap-4 text-sm">
          <div>
            <span class="text-gray-600">Network:</span>
            <span class="ml-2 font-mono text-gray-900">{{ networkInfo || 'Detecting...' }}</span>
          </div>
          <div>
            <span class="text-gray-600">Last scan:</span>
            <span class="ml-2 text-gray-900">{{ lastScanTime || 'Never' }}</span>
          </div>
        </div>
      </div>

      <!-- Devices -->
      <div class="bg-white rounded-lg shadow-sm p-6 mb-6">
        <div class="flex items-center justify-between mb-4">
          <h2 class="text-lg font-semibold text-gray-900">
            {{ devices.length }} device{{ devices.length !== 1 ? 's' : '' }} found
          </h2>
          <div class="flex gap-2">
            <button
              @click="handleHealthCheck"
              :disabled="checkingHealth || devices.length === 0"
              class="bg-emerald-600 hover:bg-emerald-700 disabled:bg-gray-400 text-white font-medium py-2 px-4 rounded-lg transition-colors"
              title="Check if devices are reachable"
            >
              {{ checkingHealth ? 'Checking...' : '🩺 Health Check' }}
            </button>
            <button
              @click="handleScan"
              :disabled="scanning"
              class="bg-indigo-600 hover:bg-indigo-700 disabled:bg-gray-400 text-white font-medium py-2 px-4 rounded-lg transition-colors"
            >
              {{ scanning ? 'Scanning...' : 'Scan Now' }}
            </button>
          </div>
        </div>

        <!-- Health Check Results -->
        <div v-if="healthStatus" class="mb-4 p-3 bg-gray-50 rounded-lg text-sm">
          <div class="flex items-center justify-between">
            <div class="flex gap-4">
              <span class="text-green-600">
                ✓ {{ healthStatus.healthyDevices }} healthy
              </span>
              <span v-if="healthStatus.unreachableDevices > 0" class="text-red-600">
                ✗ {{ healthStatus.unreachableDevices }} unreachable
              </span>
            </div>
            <span v-if="healthStatus.syncedToCloud" class="text-indigo-600 text-xs">
              ☁️ Synced
            </span>
          </div>
        </div>

        <DeviceList :devices="devices" />
      </div>

      <!-- Actions -->
      <div class="flex gap-4">
        <button
          @click="openCloud"
          class="flex-1 bg-white hover:bg-gray-50 border border-gray-300 text-gray-700 font-medium py-3 px-6 rounded-lg transition-colors"
        >
          View in Cloud →
        </button>
      </div>
    </div>
  </div>
</template>

<script setup lang="ts">
import { ref, computed, onMounted } from 'vue'
import { useAgentStore } from '@/stores/agent'
import DeviceList from '@/components/DeviceList.vue'
import { invoke } from '@tauri-apps/api/core'

interface HealthCheckStatus {
  totalDevices: number
  healthyDevices: number
  unreachableDevices: number
  syncedToCloud: boolean
}

const agentStore = useAgentStore()
const networkInfo = ref<string>('')
const lastScanTime = ref<string>('')
const checkingHealth = ref(false)
const healthStatus = ref<HealthCheckStatus | null>(null)

const status = computed(() => agentStore.status)
const devices = computed(() => agentStore.devices)
const scanning = computed(() => agentStore.scanning)

async function handleScan() {
  try {
    await agentStore.scanNow()
    await updateLastScanTime()
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

async function updateLastScanTime() {
  if (agentStore.status.lastScan) {
    const date = new Date(agentStore.status.lastScan)
    lastScanTime.value = date.toLocaleString()
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
  await updateLastScanTime()
  
  // Refresh status periodically
  setInterval(() => {
    agentStore.refreshStatus()
  }, 30000) // Every 30 seconds
})
</script>

