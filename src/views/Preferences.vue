<template>
  <div class="min-h-screen bg-gray-50">
    <div class="max-w-2xl mx-auto p-6">
      <div class="bg-white rounded-lg shadow-sm p-6 mb-6">
        <h1 class="text-2xl font-bold text-gray-900 mb-6">Preferences</h1>

        <!-- Scan Settings -->
        <div class="space-y-6">
          <div>
            <label class="block text-sm font-medium text-gray-700 mb-2">
              Scan Interval
            </label>
            <select
              v-model="selectedInterval"
              @change="updateInterval"
              class="w-full border border-gray-300 rounded-lg px-4 py-2 focus:ring-2 focus:ring-indigo-500 focus:border-indigo-500"
            >
              <option :value="1">1 minute</option>
              <option :value="5">5 minutes</option>
              <option :value="10">10 minutes</option>
              <option :value="15">15 minutes</option>
              <option :value="30">30 minutes</option>
              <option :value="60">1 hour</option>
            </select>
          </div>

          <div class="flex items-center justify-between">
            <div>
              <label class="block text-sm font-medium text-gray-700 mb-1">
                Start at Login
              </label>
              <p class="text-xs text-gray-500">Automatically start when you log in</p>
            </div>
            <input
              type="checkbox"
              v-model="startAtLogin"
              @change="updateStartAtLogin"
              class="w-5 h-5 text-indigo-600 rounded focus:ring-indigo-500"
            />
          </div>

          <div class="flex items-center justify-between">
            <div>
              <label class="block text-sm font-medium text-gray-700 mb-1">
                Show Notifications
              </label>
              <p class="text-xs text-gray-500">Get notified about network changes</p>
            </div>
            <input
              type="checkbox"
              v-model="showNotifications"
              @change="updateNotifications"
              class="w-5 h-5 text-indigo-600 rounded focus:ring-indigo-500"
            />
          </div>
        </div>
      </div>

      <!-- Account Info -->
      <div class="bg-white rounded-lg shadow-sm p-6 mb-6">
        <h2 class="text-lg font-semibold text-gray-900 mb-4">Account</h2>
        <div class="space-y-3 text-sm">
          <div>
            <span class="text-gray-600">Email:</span>
            <span class="ml-2 text-gray-900">{{ status.userEmail || 'Unknown' }}</span>
          </div>
          <div>
            <span class="text-gray-600">Agent ID:</span>
            <span class="ml-2 font-mono text-gray-900">{{ status.agentId || 'Unknown' }}</span>
          </div>
        </div>
        <button
          @click="handleLogout"
          class="mt-4 text-red-600 hover:text-red-700 font-medium text-sm"
        >
          Sign Out
        </button>
      </div>

      <!-- Back Button -->
      <button
        @click="$router.push('/dashboard')"
        class="w-full bg-gray-200 hover:bg-gray-300 text-gray-900 font-medium py-3 px-6 rounded-lg transition-colors"
      >
        Back to Dashboard
      </button>
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

async function updateStartAtLogin() {
  try {
    await invoke('set_start_at_login', { enabled: startAtLogin.value })
  } catch (error) {
    console.error('Failed to update start at login:', error)
  }
}

async function updateNotifications() {
  try {
    await invoke('set_notifications_enabled', { enabled: showNotifications.value })
  } catch (error) {
    console.error('Failed to update notifications:', error)
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

