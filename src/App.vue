<template>
  <div id="app-wrapper" class="flex flex-col h-screen">
    <TitleBar />
    <div class="flex-1 overflow-auto">
      <router-view v-if="ready" />
      <div v-else class="flex items-center justify-center h-full">
        <div class="text-center">
          <div class="text-2xl mb-2">Loading Cartographer Agent...</div>
        </div>
      </div>
    </div>
    <UpdateDialog />
    <UpdateNotificationBar />
  </div>
</template>

<script setup lang="ts">
import { ref, onMounted } from 'vue'
import TitleBar from '@/components/TitleBar.vue'
import UpdateDialog from '@/components/UpdateDialog.vue'
import UpdateNotificationBar from '@/components/UpdateNotificationBar.vue'

const ready = ref(false)

onMounted(() => {
  console.log('App.vue: Component mounted')
  ready.value = true
})
</script>

