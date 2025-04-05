import { defineConfig } from 'vite'

export default defineConfig({
  resolve: {
    conditions: ['development', 'browser']
  },
  build: {
    outDir: 'dist'
  },
  server: {
    port: 5173,
    strictPort: true
  }
})
