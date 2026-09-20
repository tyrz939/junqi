import { fileURLToPath, URL } from "node:url";
import { defineConfig } from "vitest/config";

export default defineConfig({
  base: "./",
  resolve: {
    alias: { "@": fileURLToPath(new URL("./src", import.meta.url)) },
  },
  build: {
    target: "es2022",
    sourcemap: true,
    // Two pages: the game, and the seed viewer the county generator is reviewed with.
    rollupOptions: {
      input: {
        main: fileURLToPath(new URL("./index.html", import.meta.url)),
        viewer: fileURLToPath(new URL("./viewer.html", import.meta.url)),
      },
    },
  },
  test: { environment: "node", include: ["test/**/*.test.ts"] },
});
