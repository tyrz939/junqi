import { fileURLToPath, URL } from "node:url";
import { defineConfig } from "vitest/config";

export default defineConfig({
  base: "./",
  resolve: {
    alias: { "@": fileURLToPath(new URL("./src", import.meta.url)) },
  },
  // Content lives at /data (PORT.md §2); src/data and the room templates are symlinks to it.
  server: { fs: { allow: [".."] } },
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
  // A county is seven million cells and about a second to build and prove, and most test files build one
  // in their first test; with every core busy that can pass the default five seconds. Thirty is honest.
  test: { environment: "node", include: ["test/**/*.test.ts"], testTimeout: 30000 },
});
