/** Build entry of the MCP App view bundle (`dist/mcp/storefront-parts.js`); the server wraps it into the `ui://` document. */
import { start } from "./mcp-app";
if (window.parent !== window) start();
