import { defineConfig } from "vite";
import react from "@vitejs/plugin-react";

// In development the panel API is the Rust server inside the app (port 8027)
export default defineConfig({
  plugins: [react()],
  server: { port: 5173, proxy: { "/api": "http://127.0.0.1:8027", "/thumb": "http://127.0.0.1:8027" } },
  build: { outDir: "dist", emptyOutDir: true },
});
