import { defineStore } from 'pinia'
import { ref, computed } from 'vue'
import { invoke } from '@tauri-apps/api/core'

export interface Device {
  ip: string
  mac?: string
  response_time_ms?: number
  hostname?: string
}

export interface AgentStatus {
  authenticated: boolean
  userEmail?: string
  agentId?: string
  lastScan?: string
  nextScan?: string
  deviceCount?: number
}

export const useAgentStore = defineStore('agent', () => {
  const status = ref<AgentStatus>({
    authenticated: false
  })
  
  const devices = ref<Device[]>([])
  const scanning = ref(false)
  const scanInterval = ref(5) // minutes

  const isAuthenticated = computed(() => status.value.authenticated)

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

  return {
    status,
    devices,
    scanning,
    scanInterval,
    isAuthenticated,
    checkAuth,
    login,
    logout,
    scanNow,
    refreshStatus,
    setScanInterval
  }
})

