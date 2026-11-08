import { readFileSync } from "node:fs";
import { defineConfig } from "vite";
import { svelte } from "@sveltejs/vite-plugin-svelte";

const { version } = JSON.parse(readFileSync(new URL("./src-tauri/tauri.conf.json", import.meta.url), "utf8"));

// Tauri expects a fixed dev-server port and its own files left alone.
export default defineConfig({
  plugins: [svelte()],
  define: { __APP_VERSION__: JSON.stringify(version) },
  clearScreen: false,
  server: {
    host: "127.0.0.1",
    port: 1420,
    strictPort: true,
    watch: { ignored: ["**/src-tauri/**"] },
  },
  envPrefix: ["VITE_", "TAURI_ENV_"],
  build: {
    // WebKitGTK 2.54 on Fedora 44.
    target: "safari16",
    outDir: "dist",
    emptyOutDir: true,
  },
});
