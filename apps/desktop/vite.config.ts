import solid from "@solidjs/vite-plugin";
import { defineConfig } from "vitest/config";

// Tauri expects a fixed dev port and doesn't want Vite to clear its output.
// https://v2.tauri.app/start/frontend/vite/
const host = process.env.TAURI_DEV_HOST;

export default defineConfig({
  plugins: [solid()],
  clearScreen: false,
  server: {
    port: 1420,
    strictPort: true,
    host: host || false,
    hmr: host ? { protocol: "ws", host, port: 1421 } : undefined,
    watch: { ignored: ["**/src-tauri/**"] },
  },
  envPrefix: ["VITE_", "TAURI_ENV_*"],
  build: {
    // Oldest engines we ship on: WebKitGTK / WKWebView (Safari 16 level) and WebView2.
    target: ["es2022", "safari16", "chrome120"],
    sourcemap: !!process.env.TAURI_ENV_DEBUG,
  },
  test: {
    environment: "jsdom",
    globals: false,
  },
});
