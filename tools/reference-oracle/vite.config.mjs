import { defineConfig } from "vite";

export default defineConfig({
  base: "/bundle/",
  mode: "production",
  build: {
    emptyOutDir: true,
    manifest: true,
    minify: "oxc",
    outDir: "dist",
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
