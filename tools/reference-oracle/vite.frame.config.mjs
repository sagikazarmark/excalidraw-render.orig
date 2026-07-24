import { defineConfig } from "vite";

export default defineConfig({
  base: "/frame-bundle/",
  mode: "production",
  build: {
    emptyOutDir: true,
    manifest: true,
    minify: "oxc",
    outDir: "frame-dist",
    rollupOptions: { input: "frame-index.html" },
    sourcemap: false,
    target: "chrome149",
  },
  define: {
    "process.env.NODE_ENV": JSON.stringify("production"),
  },
  worker: {
    format: "es",
  },
});
