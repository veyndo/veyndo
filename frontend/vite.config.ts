import { defineConfig } from "vite";
import react from "@vitejs/plugin-react";
export default defineConfig({
  plugins: [react()],
  resolve: {
    // Component UI folders (components/*/ui) resolve packages from this frontend.
    dedupe: [
      "react",
      "react-dom",
      "zod",
      "@veyndo/sdk",
      "@openuidev/react-lang",
    ],
    alias: {
      "three/examples/jsm/libs/meshopt_decoder.module.js": new URL(
        "./src/shared/media/model-decoder-disabled.ts",
        import.meta.url,
      ).pathname,
    },
  },
  // Open sessions may still need their earlier content-addressed lazy chunks.
  // Release packaging starts fresh; local in-place builds retain existing assets.
  build: {
    emptyOutDir: false,
    rollupOptions: {
      // The app, plus the storefront-kit web elements under stable names for embedding hosts.
      input: {
        main: new URL("./index.html", import.meta.url).pathname,
        elements: new URL("./src/kit/storefront/embed.ts", import.meta.url)
          .pathname,
      },
      output: {
        entryFileNames: (chunk) =>
          chunk.name === "elements"
            ? "assets/elements.js"
            : "assets/[name]-[hash].js",
      },
    },
  },
  server: {
    proxy: {
      "/api": "http://127.0.0.1:8787",
      "/app-ui": "http://127.0.0.1:8787",
      "/store-api": "http://127.0.0.1:8787",
      "/health": "http://127.0.0.1:8787",
    },
  },
});
