/** Catalog and product reads through the generated SDK: one client per request, shop and language explicit, no credentials. */
import { VeyndoClient } from "./sdk.ts";
import { LOCALES, type Language } from "./text.ts";

export interface Product {
  id: string;
  name: string;
  description?: string;
  price: number;
  currency: string;
  stock?: number;
  delivery_days?: number;
  media?: Array<{ url: string }>;
}

export interface Shop {
  url: string;
  shop: string;
  fetch?: typeof fetch;
}

function client(shop: Shop) {
  return new VeyndoClient({ baseUrl: shop.url, tenant: shop.shop, fetch: shop.fetch });
}

const locale = (lang: Language) => ({ "x-commerce-locale": LOCALES[lang] });

/** One catalog page (`POST /store-api/product`), localized by the shop. */
export async function catalog(shop: Shop, lang: Language, after?: string) {
  const page = (await client(shop).catalog.postStoreProduct({
    body: { limit: 24, ...(after ? { after } : {}) },
    headers: locale(lang),
  })) as { elements: Product[]; nextCursor?: string | null };
  return { products: page.elements, next: page.nextCursor ?? null };
}

/** One product (`GET /store-api/product/{id}`), with the shop's own price, stock and delivery facts. */
export async function product(shop: Shop, lang: Language, id: string) {
  const detail = (await client(shop).catalog.getStoreProductById({ id }, { headers: locale(lang) })) as { product: Product };
  return detail.product;
}

/** Media URLs from the shop may be relative to its origin. */
export function image(shop: Shop, item: Product) {
  const url = item.media?.[0]?.url;
  return url ? new URL(url, shop.url).href : null;
}
