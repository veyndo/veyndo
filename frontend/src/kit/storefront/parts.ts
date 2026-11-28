/** The list of storefront parts in builder order and lookup by id. */
import { productCard } from "./ProductCard";
import { productSummary } from "./ProductSummary";
import { cartSummary } from "./CartSummary";
import { checkoutSteps } from "./CheckoutSteps";
import { accountOverview, orderList } from "./AccountOverview";
import type { AnyPart } from "./contract";
/** Every storefront part, in builder order. */
export const parts = [
  productCard,
  productSummary,
  cartSummary,
  checkoutSteps,
  accountOverview,
  orderList,
] as const;
export type PartId = (typeof parts)[number]["id"];
/** The part with `id`, if it exists. */
export const partById = (id: string): AnyPart | undefined =>
  (parts as readonly AnyPart[]).find((part) => part.id === id);
