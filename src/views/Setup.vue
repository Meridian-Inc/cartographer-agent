<template>
  <div style="min-height: 100vh; background: linear-gradient(to bottom right, #dbeafe, #e0e7ff); display: flex; align-items: center; justify-content: center; padding: 16px;">
    <div style="background: white; border-radius: 16px; box-shadow: 0 20px 25px -5px rgba(0, 0, 0, 0.1); padding: 32px; max-width: 448px; width: 100%;">
      <div style="text-align: center; margin-bottom: 32px;">
        <div style="font-size: 60px; margin-bottom: 16px;">🗺️</div>
        <h1 style="font-size: 30px; font-weight: bold; color: #111827; margin-bottom: 8px;">Cartographer Agent</h1>
        <p style="color: #4b5563;">
          Monitor your network and sync with Cartographer Cloud automatically.
        </p>
      </div>

      <!-- Error State -->
      <div v-if="errorMessage" style="background: #fef2f2; border: 1px solid #fecaca; border-radius: 8px; padding: 12px; margin-bottom: 16px;">
        <p style="color: #dc2626; font-size: 14px; text-align: center;">{{ errorMessage }}</p>
      </div>

      <!-- Idle State -->
      <div v-if="!loggingIn" style="display: flex; flex-direction: column; gap: 16px;">
        <button
          @click="handleLogin"
          style="width: 100%; background: #4f46e5; color: white; font-weight: 600; padding: 12px 24px; border-radius: 8px; border: none; cursor: pointer;"
          onmouseover="this.style.background='#4338ca'"
          onmouseout="this.style.background='#4f46e5'"
        >
          Connect to Cloud
        </button>
        <p style="font-size: 14px; color: #6b7280; text-align: center;">
          Sign in and select a network to sync your devices
        </p>
      </div>

      <!-- Logging In State -->
      <div v-else style="display: flex; flex-direction: column; gap: 16px;">
        <div style="text-align: center;">
          <div style="display: inline-block; width: 32px; height: 32px; border: 2px solid #4f46e5; border-top-color: transparent; border-radius: 50%; margin-bottom: 16px; animation: spin 1s linear infinite;"></div>
          <p style="color: #374151; font-weight: 500;">Waiting for authorization...</p>
          <p style="font-size: 14px; color: #6b7280; margin-top: 8px;">
            Complete the sign-in and network selection in your browser.
          </p>
          <p style="font-size: 12px; color: #9ca3af; margin-top: 16px;">
            A browser window should have opened. If not, check your default browser.
          </p>
          <button
            @click="cancelLogin"
            style="margin-top: 16px; background: none; border: none; color: #6b7280; cursor: pointer; text-decoration: underline; font-size: 14px;"
          >
            Cancel
          </button>
        </div>
      </div>
    </div>
  </div>
</template>

<script setup lang="ts">
import { ref, onMounted } from 'vue'
import { useRouter } from 'vue-router'
import { useAgentStore } from '@/stores/agent'

console.log('Setup.vue: Component script executing')

const router = useRouter()
const agentStore = useAgentStore()
const loggingIn = ref(false)
const errorMessage = ref('')
let loginCancelled = false

onMounted(() => {
  console.log('Setup.vue: Component mounted')
})

async function handleLogin() {
  loggingIn.value = true
  errorMessage.value = ''
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

