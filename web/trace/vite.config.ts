import path from 'node:path'
import { defineConfig } from 'vite'
import react from '@vitejs/plugin-react'
import { viteSingleFile } from 'vite-plugin-singlefile'

export default defineConfig({
  plugins: [react(), viteSingleFile()],
  resolve: { alias: { '@': path.resolve(__dirname, './src') } },
  build: { target: 'es2020', cssCodeSplit: false, assetsInlineLimit: 100000000, chunkSizeWarningLimit: 2000 },
})
