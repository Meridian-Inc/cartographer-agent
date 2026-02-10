<template>
  <div
    v-if="updateAvailable"
    class="fixed inset-0 bg-black bg-opacity-50 flex items-center justify-center z-50"
    @click.self="dismissUpdate"
  >
    <div class="bg-white dark:bg-gray-800 rounded-lg shadow-xl max-w-md w-full mx-4">
      <div class="p-6">
        <h2 class="text-2xl font-bold mb-4 text-gray-900 dark:text-white">
          Update Available
        </h2>
        
        <div class="mb-4">
          <p class="text-gray-700 dark:text-gray-300 mb-2">
            A new version of Cartographer Agent is available:
          </p>
          <div class="flex items-center gap-2 mb-2">
            <span class="text-sm text-gray-500 dark:text-gray-400">Current:</span>
            <span class="font-mono text-sm bg-gray-100 dark:bg-gray-700 text-gray-800 dark:text-gray-200 px-2 py-1 rounded">
              {{ currentVersion }}
            </span>
            <span class="text-gray-400">→</span>
            <span class="font-mono text-sm bg-blue-100 dark:bg-blue-900 px-2 py-1 rounded text-blue-800 dark:text-blue-200">
              {{ newVersion }}
            </span>
          </div>
        </div>

        <div v-if="releaseNotes" class="mb-4">
          <h3 class="text-sm font-semibold text-gray-700 dark:text-gray-300 mb-2">
            What's New:
          </h3>
          <div
            class="changelog-content text-sm text-gray-600 dark:text-gray-400 bg-gray-50 dark:bg-gray-900 p-3 rounded max-h-40 overflow-y-auto overflow-x-hidden"
            v-html="renderedNotes"
          ></div>
        </div>

        <div v-if="isDownloading || isInstalling" class="mb-4">
          <div class="flex items-center justify-between mb-2">
            <span class="text-sm text-gray-700 dark:text-gray-300">
              {{ isDownloading ? 'Downloading update...' : 'Installing update...' }}
            </span>
            <span class="text-sm font-semibold text-gray-700 dark:text-gray-300">
              {{ downloadProgress }}%
            </span>
          </div>
          <div class="w-full bg-gray-200 dark:bg-gray-700 rounded-full h-2">
            <div
              class="bg-blue-600 dark:bg-blue-500 h-2 rounded-full transition-all duration-300"
              :style="{ width: `${downloadProgress}%` }"
            ></div>
          </div>
        </div>

        <div class="flex gap-3 mt-6">
          <button
            v-if="!isDownloading && !isInstalling"
            @click="dismissUpdate"
            class="flex-1 px-4 py-2 border border-gray-300 dark:border-gray-600 rounded-lg text-gray-700 dark:text-gray-300 hover:bg-gray-50 dark:hover:bg-gray-700 transition-colors"
          >
            Remind Me Later
          </button>
          <button
            @click="handleUpdate"
            :disabled="isDownloading || isInstalling"
            class="flex-1 px-4 py-2 bg-blue-600 text-white rounded-lg hover:bg-blue-700 disabled:opacity-50 disabled:cursor-not-allowed transition-colors"
          >
            {{ isDownloading || isInstalling ? 'Updating...' : 'Update & Restart' }}
          </button>
        </div>
      </div>
    </div>
  </div>
</template>

<script setup lang="ts">
import { ref, computed, onMounted } from 'vue'
import { marked } from 'marked'
import { useUpdater } from '@/composables/useUpdater'
import { getVersion } from '@tauri-apps/api/app'

const {
  updateAvailable,
  newVersion,
  releaseNotes,
  releaseDate,
  downloadProgress,
  isDownloading,
  isInstalling,
  downloadAndInstall,
  dismissUpdate
} = useUpdater()

const currentVersion = ref('')

const renderedNotes = computed(() => {
  if (!releaseNotes.value) return ''
  return marked.parse(releaseNotes.value, { async: false }) as string
})

onMounted(async () => {
  try {
    currentVersion.value = await getVersion()
  } catch (error) {
    console.error('Failed to get app version:', error)
    currentVersion.value = 'Unknown'
  }
})

async function handleUpdate() {
  try {
    await downloadAndInstall()
  } catch (error) {
    console.error('Update failed:', error)
    // Could show an error message to the user here
  }
}
</script>

<style scoped>
.changelog-content :deep(h1),
.changelog-content :deep(h2),
.changelog-content :deep(h3) {
  font-weight: 600;
  margin-top: 0.5rem;
  margin-bottom: 0.25rem;
}

.changelog-content :deep(h1) {
  font-size: 1rem;
}

.changelog-content :deep(h2) {
  font-size: 0.9rem;
}

.changelog-content :deep(h3) {
  font-size: 0.85rem;
}

.changelog-content :deep(ul) {
  list-style-type: disc;
  padding-left: 1.25rem;
  margin: 0.25rem 0;
}

.changelog-content :deep(li) {
  margin-bottom: 0.125rem;
}

.changelog-content :deep(a) {
  color: #3b82f6;
  text-decoration: underline;
  overflow-wrap: break-word;
  word-break: break-all;
}

.changelog-content :deep(p) {
  margin: 0.25rem 0;
  overflow-wrap: break-word;
}

.changelog-content :deep(code) {
  font-size: 0.8rem;
  background: rgba(0, 0, 0, 0.1);
  padding: 0.1rem 0.3rem;
  border-radius: 0.2rem;
}
</style>
