import { defineConfig } from "vite";
import react from "@vitejs/plugin-react";
import { resolve } from "node:path";

export default defineConfig({
  plugins: [react()],
  clearScreen: false,
  server: { port: 1420, strictPort: true },
  build: {
    target: "safari15",
    rollupOptions: {
      input: {
        index: resolve(__dirname, "index.html"),
        mascot: resolve(__dirname, "mascot.html"),
      },
    },
  },
});
