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

      <div v-if="!loggingIn" style="display: flex; flex-direction: column; gap: 16px;">
        <button
          @click="handleLogin"
          style="width: 100%; background: #4f46e5; color: white; font-weight: 600; padding: 12px 24px; border-radius: 8px; border: none; cursor: pointer;"
          onmouseover="this.style.background='#4338ca'"
          onmouseout="this.style.background='#4f46e5'"
        >
          Sign In
        </button>
        <p style="font-size: 14px; color: #6b7280; text-align: center;">
          Connect your local network to Cartographer Cloud
        </p>
      </div>

      <div v-else style="display: flex; flex-direction: column; gap: 16px;">
        <div style="text-align: center;">
          <div style="display: inline-block; width: 32px; height: 32px; border: 2px solid #4f46e5; border-top-color: transparent; border-radius: 50%; margin-bottom: 16px; animation: spin 1s linear infinite;"></div>
          <p style="color: #374151; font-weight: 500;">Opening browser for sign-in...</p>
          <p style="font-size: 14px; color: #6b7280; margin-top: 8px;">
            Complete the sign-in in your browser, then return here.
          </p>
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

onMounted(() => {
  console.log('Setup.vue: Component mounted')
})

async function handleLogin() {
  loggingIn.value = true
  try {
    const success = await agentStore.login()
    if (success) {
      router.push('/dashboard')
    }
  } catch (error) {
    console.error('Login error:', error)
    alert('Failed to sign in. Please try again.')
  } finally {
    loggingIn.value = false
  }
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

