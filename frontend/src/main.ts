import { createApp } from 'vue'
import { createPinia } from 'pinia'
import { VueQueryPlugin } from '@tanstack/vue-query'

import { onUnauthorized } from './api/http'
import App from './App.vue'
import router from './router'
import { useAuthStore } from './stores/auth'
import './styles/main.css'

const app = createApp(App)

app.use(createPinia())
app.use(VueQueryPlugin)
app.use(router)

// 非认证接口返回 401 时：清空本地登录态并回到登录页。
onUnauthorized(() => {
  const auth = useAuthStore()
  auth.clearUser()
  const current = router.currentRoute.value
  if (current.name !== 'login') {
    void router.push({ name: 'login', query: { redirect: current.fullPath } })
  }
})

app.mount('#app')
