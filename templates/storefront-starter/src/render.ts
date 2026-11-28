/** Server-rendered HTML for the catalog and product pages; every shop value is escaped, every interface word comes from text.ts. */
import { image, type Product, type Shop } from "./shop.ts";
import { LOCALES, TEXT, type Language } from "./text.ts";

export function escape(value: unknown) {
  return String(value ?? "").replace(/[&<>"']/g, (c) => ({ "&": "&amp;", "<": "&lt;", ">": "&gt;", '"': "&quot;", "'": "&#39;" })[c]!);
}

function money(lang: Language, item: Product) {
  return new Intl.NumberFormat(LOCALES[lang], { style: "currency", currency: item.currency }).format(item.price);
}

function page(lang: Language, title: string, body: string) {
  const switcher = (Object.keys(LOCALES) as Language[])
    .map((l) => `<a href="?lang=${l}"${l === lang ? ' aria-current="true"' : ""}>${l.toUpperCase()}</a>`)
    .join(" ");
  return `<!doctype html><html lang="${lang}"><head><meta charset="utf-8"><meta name="viewport" content="width=device-width,initial-scale=1">
<title>${escape(title)}</title><style>body{font:16px/1.5 system-ui,sans-serif;margin:0 auto;max-width:1100px;padding:24px;color:#142943}
.grid{display:grid;grid-template-columns:repeat(auto-fill,minmax(220px,1fr));gap:20px}.card{border:1px solid #dae5f0;border-radius:14px;padding:14px;text-decoration:none;color:inherit}
.card img,.detail img{width:100%;border-radius:10px;background:#f6f9fd}.detail{display:grid;grid-template-columns:minmax(0,1fr) minmax(0,1fr);gap:32px}
nav{display:flex;justify-content:space-between;margin-bottom:24px}@media(max-width:700px){.detail{grid-template-columns:1fr}}</style></head>
<body><nav><strong>${escape(title)}</strong><span aria-label="${escape(TEXT[lang].language)}">${switcher}</span></nav>${body}</body></html>`;
}

export function catalogPage(shop: Shop, lang: Language, products: Product[]) {
  const t = TEXT[lang];
  const cards = products.map((p) => {
    const src = image(shop, p);
    return `<a class="card" href="/product/${encodeURIComponent(p.id)}?lang=${lang}">${src ? `<img src="${escape(src)}" alt="" loading="lazy">` : ""}
<h2>${escape(p.name)}</h2><p>${escape(money(lang, p))}</p></a>`;
  });
  return page(lang, t.catalog, cards.length ? `<main class="grid">${cards.join("")}</main>` : `<p>${escape(t.empty)}</p>`);
}

export function productPage(shop: Shop, lang: Language, p: Product) {
  const t = TEXT[lang];
  const src = image(shop, p);
  const stock = (p.stock ?? 0) > 0 ? t.inStock : t.soldOut;
  const delivery = p.delivery_days ? `<p>${escape(t.delivery.replace("{days}", String(p.delivery_days)))}</p>` : "";
  return page(lang, p.name, `<p><a href="/?lang=${lang}">${escape(t.back)}</a></p><main class="detail">
${src ? `<img src="${escape(src)}" alt="">` : "<div></div>"}<div><h1>${escape(p.name)}</h1><p><strong>${escape(money(lang, p))}</strong></p>
<p>${escape(stock)}</p>${delivery}<p>${escape(p.description)}</p></div></main>`);
}

export function failurePage(lang: Language) {
  return page(lang, TEXT[lang].catalog, `<p role="alert">${escape(TEXT[lang].failed)}</p>`);
}
