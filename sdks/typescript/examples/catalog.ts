/** Example: list a shop's products page by page, then call a typed MCP tool. Run with `VEYNDO_URL=... VEYNDO_SHOP=... node examples/catalog.ts`. */
import { VeyndoClient, VeyndoError, paginate } from "../src/index.ts";

declare const process: { env: Record<string, string | undefined> };

const client = new VeyndoClient({
  baseUrl: process.env.VEYNDO_URL ?? "https://api.veyndo.ai",
  tenant: process.env.VEYNDO_SHOP ?? "lumen-story-lab-15-o-98aa4751fa",
});

try {
  for await (const product of paginate((after) => client.catalog.postStoreProduct({ body: { limit: 20, after } }))) {
    console.log((product as { id: string }).id);
  }
  const search = await client.mcp.call("catalog.search", { query: "lamp", limit: 3 });
  console.log(JSON.stringify(search, null, 2));
} catch (error) {
  if (error instanceof VeyndoError) console.error(error.status, error.kind, error.message);
  else throw error;
}
