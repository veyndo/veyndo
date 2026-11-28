/** The starter's HTTP server: `/` lists the catalog, `/product/<id>` shows one product, `?lang=` picks the interface and content language. */
import { createServer } from "node:http";
import { catalog, product, type Shop } from "./shop.ts";
import { catalogPage, failurePage, productPage } from "./render.ts";
import { language } from "./text.ts";

export function handler(shop: Shop) {
  return async (url: URL, acceptLanguage?: string) => {
    const lang = language(url.searchParams.get("lang") ?? acceptLanguage);
    const match = url.pathname.match(/^\/product\/([^/]+)$/);
    try {
      if (url.pathname === "/") return { status: 200, html: catalogPage(shop, lang, (await catalog(shop, lang)).products) };
      if (match) return { status: 200, html: productPage(shop, lang, await product(shop, lang, decodeURIComponent(match[1]))) };
      return { status: 404, html: failurePage(lang) };
    } catch {
      return { status: 502, html: failurePage(lang) };
    }
  };
}

declare const process: { env: Record<string, string | undefined>; argv: string[] };

if (import.meta.url === `file://${process.argv[1]}`) {
  const shop = { url: process.env.VEYNDO_URL ?? "https://api.veyndo.ai", shop: process.env.VEYNDO_SHOP ?? "" };
  if (!shop.shop) throw new Error("Set VEYNDO_SHOP to your shop or sandbox ID.");
  const serve = handler(shop);
  const port = Number(process.env.PORT ?? 3000);
  createServer(async (request, response) => {
    const result = await serve(new URL(request.url ?? "/", "http://localhost"), request.headers["accept-language"]);
    response.writeHead(result.status, { "content-type": "text/html; charset=utf-8" }).end(result.html);
  }).listen(port, () => console.log(`Storefront for ${shop.shop} on http://localhost:${port}`));
}
