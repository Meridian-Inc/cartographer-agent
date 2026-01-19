import { ref, onMounted, onUnmounted } from 'vue'
import { listen } from '@tauri-apps/api/event'
import { check, type DownloadEvent } from '@tauri-apps/plugin-updater'
import type { UnlistenFn } from '@tauri-apps/api/event'

export interface UpdateAvailableEvent {
  version: string
  body?: string
  date?: string
}

export function useUpdater() {
  const updateAvailable = ref(false)
  const newVersion = ref<string | null>(null)
  const releaseNotes = ref<string | null>(null)
  const releaseDate = ref<string | null>(null)
  const downloadProgress = ref(0)
  const isDownloading = ref(false)
  const isInstalling = ref(false)

  let unlisten: UnlistenFn | null = null

  onMounted(async () => {
    // Listen for update-available events from Rust backend
    unlisten = await listen<UpdateAvailableEvent>('update-available', (event) => {
      console.log('Update available event received:', event.payload)
      updateAvailable.value = true
      newVersion.value = event.payload.version
      releaseNotes.value = event.payload.body || null
      releaseDate.value = event.payload.date || null
    })
  })

  onUnmounted(() => {
    if (unlisten) {
      unlisten()
    }
  })

  async function downloadAndInstall() {
    if (!newVersion.value) {
      console.error('No update version available')
      return
    }

    try {
      isDownloading.value = true
      downloadProgress.value = 0

      // Check for update (this will get the update object)
      const update = await check()
      
      if (!update || !update.available) {
        console.log('No update available')
        updateAvailable.value = false
        return
      }

      // Track content length for progress calculation
      let totalContentLength: number | null = null

      // Download and install with progress callbacks
      await update.downloadAndInstall((progress: DownloadEvent) => {
        if (progress.event === 'Started') {
          // Store content length from Started event
          totalContentLength = progress.data?.contentLength || null
          downloadProgress.value = 0
        } else if (progress.event === 'Progress') {
          // Calculate progress based on chunk length and total content length
          const chunkLength = progress.data.chunkLength || 0
          if (totalContentLength) {
            downloadProgress.value = Math.round((chunkLength / totalContentLength) * 100)
          } else {
            // If content length is unknown, show indeterminate progress
            downloadProgress.value = Math.min(downloadProgress.value + 5, 95)
          }
        } else if (progress.event === 'Finished') {
          isDownloading.value = false
          isInstalling.value = true
          downloadProgress.value = 100
        }
      })

      // Install and restart (install() automatically restarts the app)
      await update.install()
    } catch (error) {
      console.error('Failed to download/install update:', error)
      isDownloading.value = false
      isInstalling.value = false
      throw error
    }
  }

  function dismissUpdate() {
    updateAvailable.value = false
    newVersion.value = null
    releaseNotes.value = null
    releaseDate.value = null
    downloadProgress.value = 0
  }

  return {
    updateAvailable,
    newVersion,
    releaseNotes,
    releaseDate,
    downloadProgress,
    isDownloading,
    isInstalling,
    downloadAndInstall,
    dismissUpdate
  }
}
