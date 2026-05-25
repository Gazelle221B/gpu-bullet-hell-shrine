import { defineConfig } from "vite";
import wasm from "vite-plugin-wasm";

export default defineConfig({
  plugins: [wasm()],
  root: "web",
  base: process.env.GITHUB_ACTIONS ? "/gpu-bullet-hell-shrine/" : "/",
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
