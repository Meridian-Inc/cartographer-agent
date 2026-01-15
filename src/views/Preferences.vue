<template>
  <div class="min-h-screen bg-dark-900">
    <!-- Background gradient effect -->
    <div class="absolute inset-0 bg-gradient-to-br from-brand-cyan/5 via-transparent to-brand-blue/5 pointer-events-none"></div>

    <div class="relative max-w-2xl mx-auto p-6">
      <!-- Header -->
      <div class="flex items-center gap-4 mb-6">
        <button
          @click="$router.push('/dashboard')"
          class="text-gray-400 hover:text-white p-2 rounded-lg hover:bg-dark-700 transition-colors"
        >
          <svg class="w-5 h-5" fill="none" viewBox="0 0 24 24" stroke="currentColor">
            <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M15 19l-7-7 7-7" />
          </svg>
        </button>
        <h1 class="text-2xl font-bold text-white">Preferences</h1>
      </div>

      <!-- Scan Settings -->
      <div class="bg-dark-800 border border-dark-600 rounded-xl p-6 mb-6">
        <h2 class="text-lg font-semibold text-white mb-6 flex items-center gap-2">
          <svg class="w-5 h-5 text-brand-cyan" fill="none" viewBox="0 0 24 24" stroke="currentColor">
            <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M12 6V4m0 2a2 2 0 100 4m0-4a2 2 0 110 4m-6 8a2 2 0 100-4m0 4a2 2 0 110-4m0 4v2m0-6V4m6 6v10m6-2a2 2 0 100-4m0 4a2 2 0 110-4m0 4v2m0-6V4" />
          </svg>
          Scan Settings
        </h2>

        <div class="space-y-6">
          <!-- Scan Interval -->
          <div>
            <label class="block text-sm font-medium text-gray-300 mb-2">
              Scan Interval
            </label>
            <select
              v-model="selectedInterval"
              @change="updateInterval"
              class="w-full bg-dark-700 border border-dark-600 rounded-lg px-4 py-2.5 text-white focus:ring-2 focus:ring-brand-cyan focus:border-brand-cyan transition-colors"
            >
              <option :value="1">1 minute</option>
              <option :value="5">5 minutes</option>
              <option :value="10">10 minutes</option>
              <option :value="15">15 minutes</option>
              <option :value="30">30 minutes</option>
              <option :value="60">1 hour</option>
            </select>
          </div>

          <!-- Start at Login -->
          <div class="flex items-center justify-between">
            <div>
              <label class="block text-sm font-medium text-gray-300 mb-1">
                Start at Login
              </label>
              <p class="text-xs text-gray-500">Automatically start when you log in</p>
            </div>
            <button
              @click="toggleStartAtLogin"
              :class="[
                'relative inline-flex h-6 w-11 items-center rounded-full transition-colors',
                startAtLogin ? 'bg-brand-cyan' : 'bg-dark-600'
              ]"
            >
              <span
                :class="[
                  'inline-block h-4 w-4 transform rounded-full bg-white transition-transform',
                  startAtLogin ? 'translate-x-6' : 'translate-x-1'
                ]"
              />
            </button>
          </div>

          <!-- Show Notifications -->
          <div class="flex items-center justify-between">
            <div>
              <label class="block text-sm font-medium text-gray-300 mb-1">
                Show Notifications
              </label>
              <p class="text-xs text-gray-500">Get notified about network changes</p>
            </div>
            <button
              @click="toggleNotifications"
              :class="[
                'relative inline-flex h-6 w-11 items-center rounded-full transition-colors',
                showNotifications ? 'bg-brand-cyan' : 'bg-dark-600'
              ]"
            >
              <span
                :class="[
                  'inline-block h-4 w-4 transform rounded-full bg-white transition-transform',
                  showNotifications ? 'translate-x-6' : 'translate-x-1'
                ]"
              />
            </button>
          </div>
        </div>
      </div>

      <!-- Account Info -->
      <div class="bg-dark-800 border border-dark-600 rounded-xl p-6 mb-6">
        <h2 class="text-lg font-semibold text-white mb-4 flex items-center gap-2">
          <svg class="w-5 h-5 text-brand-cyan" fill="none" viewBox="0 0 24 24" stroke="currentColor">
            <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M16 7a4 4 0 11-8 0 4 4 0 018 0zM12 14a7 7 0 00-7 7h14a7 7 0 00-7-7z" />
          </svg>
          Account
        </h2>
        <div class="space-y-3 text-sm">
          <div class="flex items-center justify-between py-2 border-b border-dark-600">
            <span class="text-gray-400">Email</span>
            <span class="text-white">{{ status.userEmail || 'Not connected' }}</span>
          </div>
          <div class="flex items-center justify-between py-2 border-b border-dark-600">
            <span class="text-gray-400">Network</span>
            <span class="text-white">{{ status.networkName || 'Not connected' }}</span>
          </div>
          <div v-if="status.networkId" class="flex items-center justify-between py-2">
            <span class="text-gray-400">Network ID</span>
            <span class="font-mono text-white text-xs">{{ status.networkId }}</span>
          </div>
        </div>
        <button
          @click="handleLogout"
          class="mt-6 text-red-400 hover:text-red-300 font-medium text-sm flex items-center gap-2 transition-colors"
        >
          <svg class="w-4 h-4" fill="none" viewBox="0 0 24 24" stroke="currentColor">
            <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M17 16l4-4m0 0l-4-4m4 4H7m6 4v1a3 3 0 01-3 3H6a3 3 0 01-3-3V7a3 3 0 013-3h4a3 3 0 013 3v1" />
          </svg>
          Sign Out
        </button>
      </div>

      <!-- About -->
      <div class="bg-dark-800 border border-dark-600 rounded-xl p-6">
        <h2 class="text-lg font-semibold text-white mb-4 flex items-center gap-2">
          <svg class="w-5 h-5 text-brand-cyan" fill="none" viewBox="0 0 24 24" stroke="currentColor">
            <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M13 16h-1v-4h-1m1-4h.01M21 12a9 9 0 11-18 0 9 9 0 0118 0z" />
          </svg>
          About
        </h2>
        <div class="text-sm text-gray-400">
          <p>Cartographer Agent</p>
          <p class="text-xs text-gray-500 mt-1">Lightweight network scanner with cloud sync</p>
        </div>
      </div>
    </div>
  </div>
</template>

<script setup lang="ts">
import { ref, computed, onMounted } from 'vue'
import { useRouter } from 'vue-router'
import { useAgentStore } from '@/stores/agent'
import { invoke } from '@tauri-apps/api/core'

const router = useRouter()
const agentStore = useAgentStore()

const status = computed(() => agentStore.status)
const selectedInterval = ref(5)
const startAtLogin = ref(false)
const showNotifications = ref(true)

async function updateInterval() {
  try {
    await agentStore.setScanInterval(selectedInterval.value)
  } catch (error) {
    console.error('Failed to update interval:', error)
  }
}

async function toggleStartAtLogin() {
  startAtLogin.value = !startAtLogin.value
  try {
    await invoke('set_start_at_login', { enabled: startAtLogin.value })
  } catch (error) {
    console.error('Failed to update start at login:', error)
    startAtLogin.value = !startAtLogin.value // Revert on error
  }
}

async function toggleNotifications() {
  showNotifications.value = !showNotifications.value
  try {
    await invoke('set_notifications_enabled', { enabled: showNotifications.value })
  } catch (error) {
    console.error('Failed to update notifications:', error)
    showNotifications.value = !showNotifications.value // Revert on error
  }
}

async function handleLogout() {
  if (confirm('Are you sure you want to sign out?')) {
    try {
      await agentStore.logout()
      router.push('/')
    } catch (error) {
      console.error('Logout error:', error)
      alert('Failed to sign out. Please try again.')
    }
  }
}

onMounted(async () => {
  await agentStore.refreshStatus()
  selectedInterval.value = agentStore.scanInterval

  try {
    startAtLogin.value = await invoke<boolean>('get_start_at_login')
    showNotifications.value = await invoke<boolean>('get_notifications_enabled')
  } catch (error) {
    console.error('Failed to load preferences:', error)
  }
})
</script>
