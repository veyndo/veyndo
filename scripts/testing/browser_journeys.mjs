/** Real-browser journeys through Studio and storefront; started by scripts/browser_journeys.py with disposable shops. */
import assert from "node:assert/strict";
import { mkdirSync, readFileSync } from "node:fs";
import path from "node:path";

const { chromium } = await import(process.env.BROWSER_MODULE);
const { base, shops } = JSON.parse(
  readFileSync(process.env.JOURNEY_FIXTURE, "utf8"),
);
assert.match(base, /^http:\/\/127\.0\.0\.1:\d+$/, "Journeys run on loopback");
const screens = process.env.JOURNEY_ARTIFACTS ?? "artifacts/browser-journeys";
mkdirSync(screens, { recursive: true });
const browser = await chromium.launch();

/** One journey in its own browser context; a failure keeps a screenshot. */
async function journey(name, locale, steps) {
  const context = await browser.newContext({ locale });
  const page = await context.newPage();
  page.setDefaultTimeout(20000);
  try {
    await steps(page);
    console.log("PASS journey:", name);
  } catch (error) {
    await page
      .screenshot({ path: path.join(screens, `${name}.png`), fullPage: true })
      .catch(() => {});
    throw error;
  } finally {
    await context.close();
  }
}

/** The shop's merchant API, for reading what the browser changed. */
async function api(page, shop, route, init = {}) {
  const response = await page.request.fetch(base + route, {
    ...init,
    headers: {
      Authorization: `Bearer ${shop.token}`,
      "x-tenant": shop.id,
      ...init.headers,
    },
  });
  assert.equal(response.status(), 200, `${route}: ${await response.text()}`);
  return response.json();
}

/** A guest order through the Store API, as another shopper would place it. */
async function storeOrder(page, shop, productId) {
  const store = async (route, data, headers, method = "POST") => {
    const response = await page.request.fetch(base + route, {
      method,
      data,
      headers: { "x-tenant": shop.id, ...headers },
    });
    assert.equal(response.status(), 200, `${route}: ${await response.text()}`);
    return response.json();
  };
  let cart = await store("/store-api/checkout/cart", {
    session: crypto.randomUUID(),
  });
  const headers = { "sw-context-token": cart.token };
  cart = await store(
    "/store-api/checkout/cart/line-item",
    { items: [{ referencedId: productId, quantity: 1 }] },
    headers,
  );
  const checkout = {
    ...cart.checkout,
    paymentMethodId: "demo-card",
    customerEmail: "parallel-buyer@example.test",
    billingAddress: {
      name: "Paul Parallel",
      firstName: "Paul",
      lastName: "Parallel",
      street: "Nebenstraße 3",
      postalCode: "10115",
      city: "Berlin",
      country: "DE",
    },
  };
  await store(
    "/store-api/checkout/context",
    { revision: cart.revision, checkout },
    headers,
    "PUT",
  );
  return store(
    "/store-api/checkout/order",
    {},
    { ...headers, "Idempotency-Key": crypto.randomUUID() },
  );
}

async function declineOptionalTracking(page) {
  const banner = page.getByRole("dialog", { name: "Datenschutzauswahl" });
  await banner.waitFor({ timeout: 10000 }).catch(() => {});
  if (await banner.isVisible())
    await banner.getByRole("button", { name: "Optionales ablehnen" }).click();
}

async function signIn(page, shop, area = "") {
  await page.goto(`${base}/?shop=${shop.id}#merchant`);
  await page.getByRole("textbox", { name: "E-Mail" }).fill(shop.email);
  await page.getByRole("textbox", { name: /^Passwort/ }).fill(shop.password);
  await page.getByRole("button", { name: "Anmelden", exact: true }).last().click();
  await page.getByRole("button", { name: "Abmelden" }).waitFor();
  if (area) await page.goto(`${base}/?shop=${shop.id}&studio=${area}#merchant`);
}

async function openOnlyOrder(page) {
  await page.getByRole("button", { name: "Details öffnen" }).first().click();
  await page.getByRole("heading", { name: /^RAC-/ }).waitFor();
}

// 1. A guest buys in the storefront; the order arrives in Studio.
await journey("purchase", "de-DE", async (page) => {
  const shop = shops.purchase;
  await page.goto(`${base}/products/coat?shop=${shop.id}`);
  await declineOptionalTracking(page);
  await page.getByRole("button", { name: "In den Warenkorb" }).click();
  const checkout = page.getByRole("dialog", { name: "Zur Kasse" });
  for (const [label, value] of [
    ["E-Mail", "guest-journey@example.test"],
    ["Vorname", "Jana"],
    ["Nachname", "Journey"],
    ["Straße und Hausnummer", "Teststraße 1"],
    ["Postleitzahl", "10115"],
    ["Ort", "Berlin"],
  ])
    await checkout.getByRole("textbox", { name: label }).fill(value);
  await checkout.getByRole("button", { name: "Bestellung prüfen" }).click();
  await checkout
    .getByRole("button", { name: /^Zahlungspflichtig bestellen · 249,00 €/ })
    .click();
  await page.getByRole("heading", { name: "Vorfreude steht dir gut." }).waitFor();
  const number = (
    await page.getByText(/^RAC-[0-9a-f]+$/).first().textContent()
  ).trim();
  await signIn(page, shop, "orders");
  await page.getByRole("button", { name: new RegExp(number) }).waitFor();
  await openOnlyOrder(page);
  assert.equal(
    await page.getByRole("heading", { name: number }).count(),
    1,
    "Studio opens the storefront's order",
  );
  await page.getByRole("heading", { name: "Eingegangen" }).waitFor();
});

// 2. The merchant refunds a captured provider payment in Studio.
await journey("refund", "de-DE", async (page) => {
  const shop = shops.refund;
  const { number, attempt, amountMinor, shopper } = shop.order;
  await signIn(page, shop, "orders");
  await openOnlyOrder(page);
  await page.getByRole("heading", { name: number }).waitFor();
  const amount = (amountMinor / 100).toFixed(2).replace(".", ",");
  await page.getByRole("textbox", { name: /^Betrag erstatten/ }).fill(amount);
  await page.getByRole("button", { name: "Betrag erstatten", exact: true }).click();
  let payment;
  for (let i = 0; i < 60; i++) {
    const response = await page.request.get(
      `${base}/store-api/payments/${attempt}`,
      { headers: shopper },
    );
    payment = response.ok() ? await response.json() : null;
    if (payment?.state === "refunded") break;
    await page.waitForTimeout(250);
  }
  assert.equal(payment?.state, "refunded", JSON.stringify(payment));
  assert.equal(payment.refundedMinor, amountMinor);
  // After a reload the order stays open and shows the refunded ledger.
  await page.reload();
  await page.getByRole("heading", { name: number }).waitFor();
  await page.getByText("Erstattet", { exact: true }).first().waitFor();
});

// 3. A content edit saves while an order changes stock; stock is never overwritten.
await journey("product-edit-while-orders-arrive", "de-DE", async (page) => {
  const shop = shops.edit;
  const before = await api(page, shop, "/api/merchant/products/bag");
  await signIn(page, shop, "productData&entity=bag");
  const name = page.getByRole("textbox", { name: "Name", exact: true });
  await name.waitFor();
  await name.fill("Everyday Ledertasche Journey");
  const order = await storeOrder(page, shop, "bag");
  assert.ok(order.orderNumber ?? order.id, "The parallel order was placed");
  await page.getByRole("button", { name: "Produkt speichern" }).click();
  await page
    .getByRole("button", { name: "Produkt speichern" })
    .and(page.locator("[disabled]"))
    .waitFor();
  const after = await api(page, shop, "/api/merchant/products/bag");
  assert.equal(after.translations.de.name, "Everyday Ledertasche Journey");
  assert.equal(
    after.commerce.stock,
    before.commerce.stock - 1,
    "The editor kept the order's stock decrement",
  );
});

// 4. The merchant installs an app after reviewing its permissions.
await journey("app-installation", "de-DE", async (page) => {
  const shop = shops.apps;
  await signIn(page, shop, "apps");
  const card = page
    .locator("article, section, li")
    .filter({
      has: page.getByRole("heading", { name: "Produkt personalisieren" }),
    })
    .last();
  await card.getByRole("button", { name: "Installieren" }).click();
  const review = page.getByRole("dialog", { name: "App-Berechtigungen prüfen" });
  await review.getByRole("heading", { name: "Bereit zur Installation" }).waitFor();
  await review.getByRole("button", { name: "Installieren" }).click();
  // Studio opens the installed app's details.
  await review.waitFor({ state: "hidden" });
  await page.getByText("Aktivität & Limits").waitFor();
  const apps = await api(page, shop, "/api/apps");
  assert.ok(
    apps.packages.some((app) => app.id === "engraving"),
    JSON.stringify(apps.packages.map((app) => app.id)),
  );
});

// 5. A Spanish main language with a fifth content language: a merchant translation
//    reaches the storefront and untranslated text inherits Spanish, not English.
await journey("non-english-main-language", "de-DE", async (page) => {
  const shop = shops.language;
  await signIn(page, shop, "productData&entity=bag");
  await page
    .getByRole("combobox", { name: "Inhaltssprache" })
    .selectOption({ label: "Italienisch" });
  const name = page.getByRole("textbox", { name: "Name", exact: true });
  await name.fill("Borsa di tutti i giorni");
  await page.getByRole("button", { name: "Produkt speichern" }).click();
  await page
    .getByRole("button", { name: "Produkt speichern" })
    .and(page.locator("[disabled]"))
    .waitFor();
  const saved = await api(page, shop, "/api/merchant/products/bag");
  const italian = Object.entries(saved.translations).find(([key]) =>
    key.startsWith("it"),
  )?.[1];
  assert.equal(italian?.name, "Borsa di tutti i giorni");
  assert.ok(
    !italian?.description,
    "An untouched field inherits; it is not saved as a fabricated translation",
  );
  const spanish = saved.translations.es;
  const shopper = await browser.newContext({ locale: "it-IT" });
  try {
    const store = await shopper.newPage();
    await store.goto(`${base}/products/bag?shop=${shop.id}&language=it-IT`);
    await store
      .getByRole("heading", { name: "Borsa di tutti i giorni", level: 1 })
      .waitFor();
    await store.getByText(spanish.description).first().waitFor();
  } finally {
    await shopper.close();
  }
});

await browser.close();
