import { defineConfig } from "vite";
import react from "@vitejs/plugin-react";

// three.js is only used by the Tools page (loaded on first open) and gets its own chunk,
// cached apart from our code. Only the `three` package itself is named: a manual chunk also
// takes in what its modules import, so naming React Three Fiber here would move React (and
// a CommonJS helper from the smaller 3D packages) out of the start bundle and make the app
// load the 3D chunks at start. R3F and drei land in the Tools page's own chunk instead.
const THREE = /[\\/]node_modules[\\/]three[\\/]/;

// In development the panel API is the Rust server inside the app (port 8027), or the Tools dev
// server `npm run tools:dev` starts (LS_API, docs/TOOLS-DEV.md)
const API = process.env.LS_API || "http://127.0.0.1:8027";

export default defineConfig({
  plugins: [react()],
  server: { port: Number(process.env.LS_VITE_PORT) || 5173, fs: { allow: ["../.."] }, proxy: { "/api": API, "/thumb": API } },
  build: {
    outDir: "dist",
    emptyOutDir: true,
    rollupOptions: {
      output: {
        manualChunks(id) {
          if (THREE.test(id)) return "three";
        },
      },
    },
  },
});
