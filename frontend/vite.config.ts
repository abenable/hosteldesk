import { defineConfig } from 'vite';
import react from '@vitejs/plugin-react';
import tailwindcss from '@tailwindcss/vite';

// Tauri loads http://localhost:5173 in dev (crates/app/tauri.conf.json devUrl)
// and embeds dist/ in release builds.
export default defineConfig({
  plugins: [react(), tailwindcss()],
  clearScreen: false,
  server: { port: 5173, strictPort: true },
  // pdf.js (the receipt viewer) is one ~500 kB chunk, loaded with import() only
  // when a receipt is first opened; nothing loaded at startup comes near this.
  build: { chunkSizeWarningLimit: 600 },
});
