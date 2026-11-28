/** Storefront starter: SDK requests name the shop and language, pages escape shop content and every interface word is translated. */
import assert from "node:assert/strict";
import { test } from "node:test";
import { handler } from "../src/server.ts";
import { TEXT } from "../src/text.ts";

const coat = { id: "coat", name: "Harbor <Wool> Coat", description: "Warm", price: 249, currency: "EUR", stock: 4, delivery_days: 3, media: [{ url: "/media/coat.webp" }] };

function fakeShop() {
  const calls = [];
  const fetch = async (url, init) => {
    calls.push({ url: String(url), method: init.method, headers: init.headers, body: init.body && JSON.parse(init.body) });
    const body = String(url).endsWith("/store-api/product") ? { elements: [coat], nextCursor: null } : { product: coat };
    return new Response(JSON.stringify(body), { status: 200, headers: { "content-type": "application/json" } });
  };
  return { shop: { url: "http://shop.test", shop: "nord-atelier", fetch }, calls };
}

test("the catalog page reads one SDK page for the shop in the chosen language", async () => {
  const { shop, calls } = fakeShop();
  const page = await handler(shop)(new URL("http://localhost/?lang=de"));
  assert.equal(page.status, 200);
  assert.equal(calls[0].url, "http://shop.test/store-api/product");
  assert.equal(calls[0].method, "POST");
  assert.equal(calls[0].headers["x-tenant"], "nord-atelier");
  assert.equal(calls[0].headers["x-commerce-locale"], "de-DE");
  assert.equal(calls[0].headers.authorization, undefined, "a storefront needs no credential");
  assert.deepEqual(calls[0].body, { limit: 24 });
  assert.match(page.html, /Harbor &lt;Wool&gt; Coat/);
  assert.match(page.html, /http:\/\/shop\.test\/media\/coat\.webp/);
  assert.match(page.html, /<html lang="de">/);
});

test("the product page shows price, stock and delivery from the shop", async () => {
  const { shop, calls } = fakeShop();
  const page = await handler(shop)(new URL("http://localhost/product/coat?lang=es"));
  assert.equal(calls[0].url, "http://shop.test/store-api/product/coat");
  assert.match(page.html, /249,00/);
  assert.match(page.html, /Disponible/);
  assert.match(page.html, /Entrega en 3 días/);
});

test("an unreachable shop gives a translated failure, not a crash", async () => {
  const shop = { url: "http://shop.test", shop: "s", fetch: async () => { throw new Error("offline"); } };
  const page = await handler(shop)(new URL("http://localhost/?lang=fr"));
  assert.equal(page.status, 502);
  assert.match(page.html, /La boutique est injoignable/);
});

test("every interface word exists in English, German, Spanish and French", () => {
  const keys = Object.keys(TEXT.en).sort();
  for (const words of Object.values(TEXT)) {
    assert.deepEqual(Object.keys(words).sort(), keys);
    assert.ok(Object.values(words).every(Boolean));
  }
});
