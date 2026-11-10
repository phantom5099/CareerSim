export const demoConfig = {
  enabled: import.meta.env.VITE_DEMO_MODE !== 'false',
  username: import.meta.env.VITE_DEMO_USERNAME ?? 'admin',
  password: import.meta.env.VITE_DEMO_PASSWORD ?? '123456',
}
