/** Build entry of the MCP App view bundle (`dist/mcp/storefront-parts.js`); the server wraps it into the `ui://` document. */
import { startCatalogMcpView } from "./mcp-view";
if (window.parent !== window) startCatalogMcpView();
