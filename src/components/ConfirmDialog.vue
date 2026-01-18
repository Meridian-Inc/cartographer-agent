<template>
  <Teleport to="body">
    <div
      v-if="visible"
      class="fixed inset-0 bg-black/60 flex items-center justify-center z-50 p-4"
      @click.self="handleCancel"
    >
      <div class="bg-dark-800 border border-dark-600 rounded-xl p-6 max-w-sm w-full">
        <h2 class="text-lg font-semibold text-white mb-3">{{ title }}</h2>
        <p class="text-gray-400 text-sm mb-6">{{ message }}</p>
        <div class="flex justify-end gap-3">
          <button
            @click="handleCancel"
            class="px-4 py-2 text-sm font-medium text-gray-400 hover:text-white transition-colors"
          >
            {{ cancelText }}
          </button>
          <button
            @click="handleConfirm"
            :class="[
              'px-4 py-2 text-sm font-medium rounded-lg transition-colors',
              destructive
                ? 'bg-red-600 hover:bg-red-500 text-white'
                : 'bg-brand-cyan hover:bg-brand-cyan/90 text-dark-900'
            ]"
          >
            {{ confirmText }}
          </button>
        </div>
      </div>
    </div>
  </Teleport>
</template>

<script setup lang="ts">
import { ref, watch } from 'vue'

const props = withDefaults(defineProps<{
  modelValue: boolean
  title?: string
  message: string
  confirmText?: string
  cancelText?: string
  destructive?: boolean
}>(), {
  title: 'Confirm',
  confirmText: 'Confirm',
  cancelText: 'Cancel',
  destructive: false
})

const emit = defineEmits<{
  'update:modelValue': [value: boolean]
  'confirm': []
  'cancel': []
}>()

const visible = ref(props.modelValue)

watch(() => props.modelValue, (newVal) => {
  visible.value = newVal
})

function handleConfirm() {
  emit('confirm')
  emit('update:modelValue', false)
  visible.value = false
}

function handleCancel() {
  emit('cancel')
  emit('update:modelValue', false)
  visible.value = false
}
</script>
