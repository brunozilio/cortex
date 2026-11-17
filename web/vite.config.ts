import path from 'node:path'
import { defineConfig } from 'vite'
import react from '@vitejs/plugin-react'

const consoleServer = process.env.CONSOLE_URL ?? 'http://127.0.0.1:3000'

export default defineConfig({
  plugins: [react()],
  resolve: { alias: { '@': path.resolve(__dirname, './src') } },
  server: { proxy: { '/api': { target: consoleServer, changeOrigin: true, ws: true } } },
  build: { target: 'es2020', chunkSizeWarningLimit: 2000 },
})
