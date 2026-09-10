import { defineConfig } from 'vite'
import react from '@vitejs/plugin-react'
import tailwindcss from '@tailwindcss/vite'

export default defineConfig({
  plugins: [react(), tailwindcss()],
  server: {
    // Dev-only proxy: the browser talks to Vite (:5173), Vite forwards
    // /api/* to the Rust API. Same-origin in the browser => no CORS needed.
    proxy: {
      '/api': 'http://localhost:8080',
    },
  },
})
