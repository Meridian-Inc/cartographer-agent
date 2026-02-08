import { createApp } from 'vue'
import { createPinia } from 'pinia'
import App from './App.vue'
import router from './router'
import { usePostHog } from './composables/usePostHog'
import './style.css'

// Disable browser-style right-click context menu
document.addEventListener('contextmenu', (e) => e.preventDefault())

const { posthog } = usePostHog()

router.afterEach((to) => {
  posthog.capture('$pageview', {
    app: 'cartographer-agent',
    route_name: String(to.name || ''),
    path: to.fullPath,
  })
})

console.log('main.ts: Starting app initialization')

try {
  const app = createApp(App)
  console.log('main.ts: Vue app created')

  app.use(createPinia())
  console.log('main.ts: Pinia added')

  app.use(router)
  console.log('main.ts: Router added')

  app.mount('#app')
  console.log('main.ts: App mounted to #app')
} catch (error) {
  console.error('main.ts: Error initializing app:', error)
  posthog.capture('frontend_init_error', {
    app: 'cartographer-agent',
    message: error instanceof Error ? error.message : String(error),
  })
  // Fallback: show error message
  const errorMessage = error instanceof Error ? error.message : String(error)
  const errorStack = error instanceof Error ? error.stack : ''
  document.body.innerHTML = `
    <div style="padding: 20px; font-family: sans-serif;">
      <h1>Error Loading App</h1>
      <p>${errorMessage}</p>
      <pre>${errorStack}</pre>
    </div>
  `
}
