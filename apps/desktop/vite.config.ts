import { defineConfig } from "vite";
import react from "@vitejs/plugin-react";

// Tauri expects a fixed port and no clearing of the screen so its logs show.
export default defineConfig({
  plugins: [react()],
  clearScreen: false,
  server: {
    port: 5173,
    strictPort: true,
  },
  // Tauri uses a modern webview; target evergreen.
  build: {
    target: "es2021",
    minify: "esbuild",
    sourcemap: false,
  },
});
