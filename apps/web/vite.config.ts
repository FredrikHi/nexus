import { defineConfig } from 'vite'
import react from '@vitejs/plugin-react'
import tailwindcss from '@tailwindcss/vite'

export default defineConfig({
  plugins: [react(), tailwindcss()],
  server: {
    // Dev-only proxy: the browser talks to Vite (:5173), Vite forwards
    // /api/* to the Rust API. Same-origin in the browser => no CORS needed.
    // Two backends, both proxied so the browser sees one origin and no CORS
    // or third-party-cookie problems exist in development.
    // More specific prefix first: /api/auth must not fall through to the API.
    proxy: {
      '/api/auth': 'http://localhost:3010',
      '/api/v1': 'http://localhost:8080',
    },
  },
})
