import { defineConfig } from "vite";
import react from "@vitejs/plugin-react";

// three.js and React Three Fiber are only used by the Tools page (loaded on first open):
// they get their own chunks, cached apart from our code. Only packages the 3D stack alone
// uses are named here, so nothing the start bundle needs (react, zustand) moves into them.
const THREE_ONLY = /[\\/]node_modules[\\/](three|three-stdlib|three-mesh-bvh|troika-[^\\/]+|bidi-js|webgl-sdf-generator|@monogrid|meshline|camera-controls|maath|stats-gl|stats\.js|detect-gpu|hls\.js)[\\/]/;
const R3F = /[\\/]node_modules[\\/](@react-three|@react-spring|@use-gesture|its-fine|suspend-react|react-reconciler|tunnel-rat|react-use-measure|debounce)[\\/]/;

// In development the panel API is the Rust server inside the app (port 8027)
export default defineConfig({
  plugins: [react()],
  server: { port: 5173, fs: { allow: ["../.."] }, proxy: { "/api": "http://127.0.0.1:8027", "/thumb": "http://127.0.0.1:8027" } },
  build: {
    outDir: "dist",
    emptyOutDir: true,
    rollupOptions: {
      output: {
        manualChunks(id) {
          if (THREE_ONLY.test(id)) return "three";
          if (R3F.test(id)) return "r3f";
        },
      },
    },
  },
});
