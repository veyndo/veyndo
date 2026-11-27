/** Storefront kickers and product details name the shop's brand, never the platform brand. */
import { render, screen } from "@testing-library/react";
import { expect, it, vi } from "vitest";
import CollectionView from "../../src/storefront/shell/CollectionView";
import ProductDetails from "../../../components/catalog/ui/storefront/catalog/ProductDetails";
import { StorefrontContext } from "../../src/storefront/shell/StorefrontContext";
import type { StorefrontController } from "../../src/storefront/shell/useStorefrontController";
import {
  brandKicker,
  shopBrandName,
  ShopBrandProvider,
} from "../../src/shared/content/shop-brand";
import type { Product } from "../../src/shared/api/shop-api";
import { de } from "../../src/shared/i18n/locales/shop-de";
vi.mock("../../src/shared/api/shop-api", async (original) => ({
  ...(await original<object>()),
  shopApi: vi.fn().mockResolvedValue({ elements: [] }),
}));

it("prefers the configured brand, then the company name, then the shop", () => {
  expect(shopBrandName({ brandName: "Hafen", name: "Hafen GmbH" }, "s")).toBe(
    "Hafen",
  );
  expect(shopBrandName({ brandName: " ", name: "Hafen GmbH" }, "s")).toBe(
    "Hafen GmbH",
  );
  expect(shopBrandName({}, "hafen-shop", "Nord Atelier")).toBe("Nord Atelier");
  expect(shopBrandName({}, "hafen-shop")).toBe("hafen-shop");
  expect(brandKicker("", "Jacken")).toBe("Jacken");
});

it("renders the shop brand in the collection kicker", () => {
  const value = {
    s: (key: keyof typeof de) => de[key] ?? key,
    locale: "de-DE",
    brand: "Hafen",
    category: "all",
    setCategory: vi.fn(),
    salesChannel: "default",
    adapted: false,
    query: "",
    setQuery: vi.fn(),
    catalogLoading: false,
    list: [],
    products: [],
    setViewed: vi.fn(),
    busy: false,
    run: vi.fn(),
    catalog: vi.fn(),
    nextCursor: null,
  } as unknown as StorefrontController;
  const { container } = render(
    <StorefrontContext.Provider value={value}>
      <CollectionView />
    </StorefrontContext.Provider>,
  );
  expect(container.querySelector(".shop-kicker")?.textContent).toBe(
    `Hafen / ${de.collection}`,
  );
  expect(container.textContent).not.toMatch(/VEYNDO/i);
});

it("renders the shop brand in the product footnote", () => {
  const product = {
    id: "coat",
    name: "Coat",
    description: "Warm",
    properties: { material: "wool" },
    options: {},
    extra: {},
  } as unknown as Product;
  render(
    <ShopBrandProvider value="Hafen">
      <ProductDetails product={product} language="de-DE" preview />
    </ShopBrandProvider>,
  );
  const note = document.querySelector(".product-material")?.textContent ?? "";
  expect(note).toMatch(/Hafen \/$/);
  expect(note).not.toMatch(/VEYNDO/i);
  expect(screen.getByText("Warm")).toBeTruthy();
});
