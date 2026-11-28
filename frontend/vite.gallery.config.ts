/** Build of the standalone component gallery (`dist/gallery/`) that the nightly browser check renders for contrast and visual snapshots. */
import { defineConfig } from "vite";
import react from "@vitejs/plugin-react";
export default defineConfig({
  plugins: [react()],
  // OpenUI's parser carries opt-in Node telemetry whose `ci-info` import reads `process`.
  resolve: {
    alias: {
      "ci-info": new URL(
        "./src/kit/catalog/ci-info-disabled.ts",
        import.meta.url,
      ).pathname,
    },
  },
  publicDir: false,
  define: { "process.env.NODE_ENV": JSON.stringify("production") },
  build: {
    outDir: "dist/gallery",
    emptyOutDir: true,
    lib: {
      entry: new URL("./src/kit/catalog/gallery-entry.tsx", import.meta.url)
        .pathname,
      formats: ["iife"],
      name: "VeyndoComponentGallery",
      fileName: () => "gallery.js",
    },
  },
});
