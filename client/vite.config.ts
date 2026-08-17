import { defineConfig } from "vite";
import react from "@vitejs/plugin-react";
import { fileURLToPath, URL } from "node:url";

export default defineConfig({
  // The root. The API moved under `/api` in D144, which is what made this
  // safe: an unmatched endpoint is answered by that scope's own 404 before the
  // fallback here is reached. See crates/server/src/assets.rs.
  base: "/",
  plugins: [react()],
  resolve: {
    alias: {
      "@design": fileURLToPath(new URL("./design", import.meta.url)),
      "@domain": fileURLToPath(new URL("./domain", import.meta.url)),
      "@app": fileURLToPath(new URL("./app", import.meta.url)),
    },
  },
  css: {
    modules: {
      // Readable in devtools, hashed enough to stay scoped. The light solver
      // binds to `data-material`, never to these names — see D123.
      generateScopedName: "ny-[local]-[hash:base64:4]",
    },
  },
});
