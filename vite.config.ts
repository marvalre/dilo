import { defineConfig } from "vite";
import react from "@vitejs/plugin-react";

export default defineConfig({
  plugins: [react()],
  clearScreen: false,
  server: { port: 1420, strictPort: true },
  build: {
    target: "safari15",
    rollupOptions: {
      input: {
        index: decodeURIComponent(new URL("./index.html", import.meta.url).pathname),
        mascot: decodeURIComponent(new URL("./mascot.html", import.meta.url).pathname),
      },
    },
  },
});
