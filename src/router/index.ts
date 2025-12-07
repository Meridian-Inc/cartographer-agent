import { createRouter, createWebHistory } from 'vue-router'

console.log('router/index.ts: Creating router')

const router = createRouter({
  history: createWebHistory(),
  routes: [
    {
      path: '/',
      name: 'setup',
      component: () => {
        console.log('router: Loading Setup.vue')
        return import('@/views/Setup.vue')
      },
      meta: { requiresAuth: false }
    },
    {
      path: '/dashboard',
      name: 'dashboard',
      component: () => {
        console.log('router: Loading Dashboard.vue')
        return import('@/views/Dashboard.vue')
      },
      meta: { requiresAuth: true }
    },
    {
      path: '/preferences',
      name: 'preferences',
      component: () => {
        console.log('router: Loading Preferences.vue')
        return import('@/views/Preferences.vue')
      },
      meta: { requiresAuth: true }
    }
  ]
})

console.log('router/index.ts: Router created')

// Navigation guard to check authentication
router.beforeEach((to, from, next) => {
  console.log('router: Navigating from', from.path, 'to', to.path)
  // Allow all navigation - check auth in components
  next()
})

export default router

