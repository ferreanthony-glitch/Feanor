import { defineConfig } from "vite";
import react from "@vitejs/plugin-react";
import tailwindcss from "@tailwindcss/vite";

// Serveur de développement que la fenêtre Tauri affiche (devUrl de tauri.conf.json).
export default defineConfig({
  plugins: [react(), tailwindcss()],
  // Laisser les messages de Rust visibles dans le terminal.
  clearScreen: false,
  server: {
    port: 5173,
    strictPort: true,
    watch: { ignored: ["**/src-tauri/**"] },
  },
  envPrefix: ["VITE_", "TAURI_ENV_*"],
  // WebView2 (Windows) est un Chromium récent.
  build: { target: "chrome110", sourcemap: false },
});
