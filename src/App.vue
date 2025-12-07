<template>
  <div id="app-wrapper">
    <router-view v-if="ready" />
    <div v-else class="flex items-center justify-center h-screen">
      <div class="text-center">
        <div class="text-2xl mb-2">Loading Cartographer Agent...</div>
      </div>
    </div>
  </div>
</template>

<script setup lang="ts">
import { ref, onMounted } from 'vue'

const ready = ref(false)

onMounted(() => {
  console.log('App.vue: Component mounted')
  ready.value = true
})
</script>

