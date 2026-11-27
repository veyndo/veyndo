/** Shop brand shown in storefront kickers, breadcrumbs and titles; the platform brand never stands in for it. */
import { createContext, useContext } from "react";

/** Public company identity fields as `/store-api/company` returns them. */
export type ShopIdentity = { brandName?: string; name?: string };

/**
 * The merchant's configured brand, then the company name, then the shop ID.
 * `demoBrand` names the bundled demo catalogue when the shop has no identity yet.
 */
export function shopBrandName(
  company: ShopIdentity,
  shop: string,
  demoBrand?: string,
): string {
  return (
    company.brandName?.trim() ||
    company.name?.trim() ||
    demoBrand?.trim() ||
    shop
  );
}

const ShopBrandContext = createContext("");

export const ShopBrandProvider = ShopBrandContext.Provider;

/** Brand of the mounted shop; empty where no storefront or preview supplies one. */
export function useShopBrand(): string {
  return useContext(ShopBrandContext);
}

/** "Brand / section" kicker; without a known brand only the section is shown. */
export function brandKicker(brand: string, section: string): string {
  return brand ? `${brand} / ${section}` : section;
}
