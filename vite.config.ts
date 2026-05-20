import { defineConfig } from "vite";
import wasm from "vite-plugin-wasm";

export default defineConfig({
  plugins: [wasm()],
  root: "web",
  build: {
    target: "esnext",
    outDir: "../dist",
    emptyOutDir: true,
  },
  server: {
    fs: {
      allow: [".."],
    },
  },
});
