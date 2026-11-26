/** The storefront language picker names each language in its own language and stays short. */
import { expect, it } from "vitest";
import { languageLabel } from "../../src/storefront/shell/StorefrontLanguage";

const shop = ["en-GB", "de-DE", "es-ES", "fr-FR"];

it("uses short autonyms when each language has one region", () => {
  expect(shop.map((key) => languageLabel(key, shop))).toEqual([
    "English",
    "Deutsch",
    "Español",
    "Français",
  ]);
});

it("keeps the region when two enabled locales share a language", () => {
  const both = ["de-DE", "de-CH"];
  expect(languageLabel("de-DE", both)).toMatch(/Deutsch \(Deutschland\)/);
  expect(languageLabel("de-CH", both)).toMatch(/Schweiz/);
});
