/** Public interface of kit/storefront: part contracts and the parts they define. */
import "../tokens";
import "./storefront.css";
import { productCard } from "./ProductCard";
import { productSummary } from "./ProductSummary";
import { cartSummary } from "./CartSummary";
import { checkoutSteps } from "./CheckoutSteps";
import { accountOverview } from "./AccountOverview";
import type { AnyPart } from "./contract";
export { definePart } from "./contract";
export type { AnyPart, Part, PartDefinition, PartEvents } from "./contract";
export { storefrontText, money } from "./storefront-i18n";
export {
  registerPartElements,
  addToCartEvent,
  checkoutEvent,
} from "./elements";
export { PartForm } from "./PartForm";
export type { PartFormProps } from "./PartForm";
export {
  productCard,
  productSummary,
  cartSummary,
  checkoutSteps,
  accountOverview,
};
/** Every storefront part, in builder order. */
export const parts = [
  productCard,
  productSummary,
  cartSummary,
  checkoutSteps,
  accountOverview,
] as const;
export type PartId = (typeof parts)[number]["id"];
/** The part with `id`, if it exists. */
export const partById = (id: string): AnyPart | undefined =>
  (parts as readonly AnyPart[]).find((part) => part.id === id);
