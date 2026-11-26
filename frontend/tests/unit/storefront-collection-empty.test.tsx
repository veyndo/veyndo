/** The collection says when a search or category has no products and offers to clear the search. */
import { fireEvent, render, renderHook, screen } from "@testing-library/react";
import { expect, it, vi } from "vitest";
import CollectionView from "../../src/storefront/shell/CollectionView";
import { useCatalog } from "../../src/storefront/shell/useCatalog";
import { StorefrontContext } from "../../src/storefront/shell/StorefrontContext";
import type { StorefrontController } from "../../src/storefront/shell/useStorefrontController";
import { de } from "../../src/shared/i18n/locales/shop-de";
vi.mock("../../src/shared/api/shop-api", async (original) => ({
  ...(await original<object>()),
  shopApi: vi.fn().mockResolvedValue({ elements: [] }),
}));

function mount(overrides: Partial<StorefrontController>) {
  const setQuery = vi.fn();
  const value = {
    s: (key: keyof typeof de) => de[key] ?? key,
    locale: "de-DE",
    category: "all",
    setCategory: vi.fn(),
    salesChannel: "default",
    adapted: false,
    query: "",
    setQuery,
    catalogLoading: false,
    list: [],
    products: [],
    setViewed: vi.fn(),
    busy: false,
    run: vi.fn(),
    catalog: vi.fn(),
    nextCursor: null,
    ...overrides,
  } as unknown as StorefrontController;
  render(
    <StorefrontContext.Provider value={value}>
      <CollectionView />
    </StorefrontContext.Provider>,
  );
  return setQuery;
}

it("explains an empty search result and clears the search", () => {
  const setQuery = mount({ query: "Wolle" });
  expect(screen.getByText(/Keine Produkte passen/)).toBeTruthy();
  fireEvent.click(screen.getByRole("button", { name: de.clearSearch }));
  expect(setQuery).toHaveBeenCalledWith("");
});

it("shows no empty state while the first page is still loading", () => {
  mount({ catalogLoading: true });
  expect(screen.queryByText(/Keine Produkte passen/)).toBeNull();
});

it("counts as loading before the first catalogue page was requested", () => {
  const { result } = renderHook(() => useCatalog(undefined, "de-DE", vi.fn()));
  expect(result.current.catalogLoading).toBe(true);
});
