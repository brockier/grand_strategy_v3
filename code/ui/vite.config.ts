import { defineConfig } from "vite";
import path from "node:path";

export default defineConfig({
  root: ".",
  base: "./",
  build: {
    outDir: "dist",
    emptyOutDir: true,
    target: "es2022",
    rollupOptions: {
      input: path.resolve(__dirname, "index.html"),
    },
  },
  test: {
    environment: "node",
    include: ["tests/**/*.test.ts"],
    testTimeout: 30000,
  },
});
