import { defineStore } from 'pinia'
import { ref, computed } from 'vue'
import { invoke } from '@tauri-apps/api/core'
import { listen, type UnlistenFn } from '@tauri-apps/api/event'

export interface Device {
  ip: string
  mac?: string
  response_time_ms?: number
  hostname?: string
}

export interface AgentStatus {
  authenticated: boolean
  userEmail?: string
  networkId?: string
  networkName?: string
  lastScan?: string
  nextScan?: string
  deviceCount?: number
}

export type ScanStage =
  | 'detecting_network'
  | 'reading_arp'
  | 'ping_sweep'
  | 'resolving_hostnames'
  | 'complete'
  | 'failed'

export interface ScanProgress {
  stage: ScanStage
  message: string
  percent: number | null
  devicesFound: number | null
  elapsedSecs: number
}

export type HealthCheckStage = 'starting' | 'checking_devices' | 'uploading' | 'complete'

export interface HealthCheckProgress {
  stage: HealthCheckStage
  message: string
  totalDevices: number
  checkedDevices: number
  healthyDevices: number
}

export const useAgentStore = defineStore('agent', () => {
  const status = ref<AgentStatus>({
    authenticated: false
  })
  
  const devices = ref<Device[]>([])
  const scanning = ref(false)
  const scanInterval = ref(5) // minutes
  const scanProgress = ref<ScanProgress | null>(null)
  const healthCheckProgress = ref<HealthCheckProgress | null>(null)

  // Event listener cleanup
  let progressUnlisten: UnlistenFn | null = null
  let healthUnlisten: UnlistenFn | null = null

  const isAuthenticated = computed(() => status.value.authenticated)

  // Initialize event listeners
  async function initEventListeners() {
    // Listen for scan progress events
    progressUnlisten = await listen<ScanProgress>('scan-progress', (event) => {
      scanProgress.value = event.payload
      // Clear progress when scan completes
      if (event.payload.stage === 'complete' || event.payload.stage === 'failed') {
        setTimeout(() => {
          scanProgress.value = null
        }, 3000) // Keep final message visible for 3 seconds
      }
    })

    // Listen for health check progress events
    healthUnlisten = await listen<HealthCheckProgress>('health-check-progress', (event) => {
      healthCheckProgress.value = event.payload
      // Clear progress when health check completes
      if (event.payload.stage === 'complete') {
        setTimeout(() => {
          healthCheckProgress.value = null
        }, 3000) // Keep final message visible for 3 seconds
      }
    })
  }

  // Cleanup event listeners
  function cleanupEventListeners() {
    if (progressUnlisten) {
      progressUnlisten()
      progressUnlisten = null
    }
    if (healthUnlisten) {
      healthUnlisten()
      healthUnlisten = null
    }
  }

  async function checkAuth() {
    try {
      const result = await invoke<AgentStatus>('check_auth_status')
      status.value = result
      return result.authenticated
    } catch (error) {
      console.error('Failed to check auth status:', error)
      return false
    }
  }

  async function login() {
    try {
      const result = await invoke<AgentStatus>('start_login_flow')
      status.value = result
      return result.authenticated
    } catch (error) {
      console.error('Login failed:', error)
      throw error
    }
  }

  async function logout() {
    try {
      await invoke('logout')
      status.value = { authenticated: false }
      devices.value = []
    } catch (error) {
      console.error('Logout failed:', error)
      throw error
    }
  }

  async function scanNow() {
    scanning.value = true
    scanProgress.value = null
    try {
      const result = await invoke<Device[]>('scan_network')
      devices.value = result
      await refreshStatus()
      return result
    } catch (error) {
      console.error('Scan failed:', error)
      throw error
    } finally {
      scanning.value = false
    }
  }

  async function refreshStatus() {
    try {
      const result = await invoke<AgentStatus>('get_agent_status')
      status.value = result
    } catch (error) {
      console.error('Failed to refresh status:', error)
    }
  }

  async function setScanInterval(minutes: number) {
    scanInterval.value = minutes
    try {
      await invoke('set_scan_interval', { minutes })
    } catch (error) {
      console.error('Failed to set scan interval:', error)
      throw error
    }
  }

  // Update devices with new data (e.g., after health check)
  function updateDevices(newDevices: Device[]) {
    devices.value = newDevices
  }

  // Initialize listeners on store creation
  initEventListeners()

  return {
    status,
    devices,
    scanning,
    scanInterval,
    scanProgress,
    healthCheckProgress,
    isAuthenticated,
    checkAuth,
    login,
    logout,
    scanNow,
    refreshStatus,
    setScanInterval,
    updateDevices,
    cleanupEventListeners
  }
})

