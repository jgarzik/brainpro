import { defineConfig } from 'vite'
import react from '@vitejs/plugin-react'
import tailwindcss from '@tailwindcss/vite'
import path from 'path'

// Gateway port - must match GATEWAY_PORT in src/constants/api.ts
const GATEWAY_PORT = 18789
const DEV_SERVER_PORT = 5173

export default defineConfig({
  plugins: [react(), tailwindcss()],
  resolve: {
    alias: {
      '@': path.resolve(__dirname, './src'),
    },
  },
  server: {
    port: DEV_SERVER_PORT,
    proxy: {
      '/ws': {
        target: `ws://localhost:${GATEWAY_PORT}`,
        ws: true,
      },
    },
  },
})
