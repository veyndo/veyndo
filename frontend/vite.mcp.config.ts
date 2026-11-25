/** Build of the MCP App view: one self-contained script the server wraps into the `ui://veyndo/kit/storefront` document. */
import { defineConfig } from "vite";
import react from "@vitejs/plugin-react";
export default defineConfig({
  plugins: [react()],
  define: { "process.env.NODE_ENV": JSON.stringify("production") },
  build: {
    outDir: "dist/mcp",
    emptyOutDir: true,
    lib: {
      entry: new URL("./src/kit/storefront/mcp-entry.ts", import.meta.url)
        .pathname,
      formats: ["iife"],
      name: "VeyndoStorefrontParts",
      fileName: () => "storefront-parts.js",
    },
  },
});
