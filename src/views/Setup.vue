<template>
  <div class="min-h-screen bg-dark-900 flex items-center justify-center p-4">
    <!-- Background gradient effect -->
    <div class="absolute inset-0 bg-gradient-to-br from-brand-cyan/5 via-transparent to-brand-blue/5 pointer-events-none"></div>

    <div class="relative bg-dark-800 border border-dark-600 rounded-xl shadow-2xl p-8 max-w-md w-full">
      <!-- Logo and Title -->
      <div class="text-center mb-8">
        <div class="w-16 h-16 mx-auto mb-4 bg-gradient-to-br from-brand-cyan to-brand-blue rounded-xl flex items-center justify-center">
          <svg class="w-10 h-10 text-white" fill="none" viewBox="0 0 24 24" stroke="currentColor">
            <path stroke-linecap="round" stroke-linejoin="round" stroke-width="1.5" d="M9 20l-5.447-2.724A1 1 0 013 16.382V5.618a1 1 0 011.447-.894L9 7m0 13l6-3m-6 3V7m6 10l4.553 2.276A1 1 0 0021 18.382V7.618a1 1 0 00-.553-.894L15 4m0 13V4m0 0L9 7" />
          </svg>
        </div>
        <h1 class="text-2xl font-bold text-white mb-2">Cartographer Agent</h1>
        <p class="text-gray-400 text-sm">
          Monitor your network and sync with Cartographer Cloud automatically.
        </p>
      </div>

      <!-- Error State -->
      <div v-if="errorMessage" class="bg-red-500/10 border border-red-500/30 rounded-lg p-3 mb-6">
        <p class="text-red-400 text-sm text-center">{{ errorMessage }}</p>
      </div>

      <!-- Idle State -->
      <div v-if="!loggingIn" class="space-y-4">
        <button
          @click="handleLogin"
          class="w-full bg-brand-cyan hover:bg-brand-cyan/90 text-dark-900 font-semibold py-3 px-6 rounded-lg transition-all duration-200 flex items-center justify-center gap-2"
        >
          <svg class="w-5 h-5" fill="none" viewBox="0 0 24 24" stroke="currentColor">
            <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M3 15a4 4 0 004 4h9a5 5 0 10-.1-9.999 5.002 5.002 0 10-9.78 2.096A4.001 4.001 0 003 15z" />
          </svg>
          Connect to Cloud
        </button>
        <p class="text-xs text-gray-500 text-center">
          Sign in and select a network to sync your devices
        </p>
      </div>

      <!-- Logging In State -->
      <div v-else class="space-y-4">
        <div class="text-center">
          <div class="inline-block w-10 h-10 border-2 border-brand-cyan border-t-transparent rounded-full mb-4 animate-spin"></div>
          <p class="text-white font-medium">Waiting for authorization...</p>
          <p class="text-sm text-gray-400 mt-2">
            Complete the sign-in and network selection in your browser.
          </p>
          <p v-if="!verificationUrl" class="text-xs text-gray-500 mt-4">
            A browser window should have opened. If not, check your default browser.
          </p>
          <div v-else class="mt-4 p-3 bg-dark-700 rounded-lg">
            <p class="text-xs text-gray-400 mb-2">
              If your browser didn't open automatically, click below:
            </p>
            <button
              @click="openVerificationUrl"
              class="text-brand-cyan hover:text-brand-cyan/80 text-sm underline transition-colors break-all"
            >
              {{ verificationUrl }}
            </button>
          </div>
          <button
            @click="cancelLogin"
            class="mt-4 text-gray-400 hover:text-white text-sm transition-colors"
          >
            Cancel
          </button>
        </div>
      </div>

      <!-- Footer -->
      <div class="mt-8 pt-6 border-t border-dark-600">
        <p class="text-xs text-gray-500 text-center">
          Lightweight network scanner with cloud sync
        </p>
      </div>
    </div>
  </div>
</template>

<script setup lang="ts">
import { ref, onMounted, onUnmounted } from 'vue'
import { useRouter } from 'vue-router'
import { useAgentStore } from '@/stores/agent'
import { listen, type UnlistenFn } from '@tauri-apps/api/event'
import { open } from '@tauri-apps/plugin-shell'

console.log('Setup.vue: Component script executing')

interface LoginUrlEvent {
  verificationUrl: string
  userCode: string
}

const router = useRouter()
const agentStore = useAgentStore()
const loggingIn = ref(false)
const errorMessage = ref('')
const verificationUrl = ref('')
let loginCancelled = false
let loginUrlUnlisten: UnlistenFn | null = null

onMounted(async () => {
  console.log('Setup.vue: Component mounted')

  // Listen for login URL events
  loginUrlUnlisten = await listen<LoginUrlEvent>('login-url', (event) => {
    console.log('Setup.vue: Received login URL:', event.payload.verificationUrl)
    verificationUrl.value = event.payload.verificationUrl
  })
})

onUnmounted(() => {
  if (loginUrlUnlisten) {
    loginUrlUnlisten()
    loginUrlUnlisten = null
  }
})

async function openVerificationUrl() {
  if (verificationUrl.value) {
    try {
      await open(verificationUrl.value)
    } catch (error) {
      console.error('Failed to open URL:', error)
    }
  }
}

async function handleLogin() {
  loggingIn.value = true
  errorMessage.value = ''
  verificationUrl.value = ''
  loginCancelled = false

  try {
    const success = await agentStore.login()
    if (loginCancelled) {
      return
    }
    if (success) {
      router.push('/dashboard')
    }
  } catch (error) {
    if (loginCancelled) {
      return
    }
    console.error('Login error:', error)
    const msg = error instanceof Error ? error.message : 'Unknown error'
    if (msg.includes('expired')) {
      errorMessage.value = 'Connection timed out. Please try again.'
    } else {
      errorMessage.value = `Failed to connect: ${msg}`
    }
  } finally {
    loggingIn.value = false
  }
}

function cancelLogin() {
  loginCancelled = true
  loggingIn.value = false
  errorMessage.value = ''
  verificationUrl.value = ''
}

// Check if already authenticated on mount
agentStore.checkAuth().then((authenticated) => {
  if (authenticated) {
    router.push('/dashboard')
  }
}).catch((error) => {
  console.warn('Failed to check auth on mount:', error)
  // Continue showing setup screen
})
</script>
