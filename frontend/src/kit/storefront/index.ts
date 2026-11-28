/** Public interface of kit/storefront: part contracts and the parts they define. */
import "../tokens";
import "./storefront.css";
import { productCard } from "./ProductCard";
import { productSummary } from "./ProductSummary";
import { cartSummary } from "./CartSummary";
import { checkoutSteps } from "./CheckoutSteps";
import { accountOverview, orderList } from "./AccountOverview";
export { definePart } from "./contract";
export type { AnyPart, Part, PartDefinition, PartEvents } from "./contract";
export { storefrontText, money, day } from "./storefront-i18n";
export {
  registerPartElements,
  addToCartEvent,
  checkoutEvent,
  quantityEvent,
  openEvent,
  selectEvent,
} from "./elements";
export { PartForm } from "./PartForm";
export { start as startMcpView, channel as mcpChannel } from "./mcp-app";
export type { RenderMeta } from "./mcp-app";
export type { PartFormProps } from "./PartForm";
export {
  productCard,
  productSummary,
  cartSummary,
  checkoutSteps,
  accountOverview,
  orderList,
};
export { parts, partById } from "./parts";
export type { PartId } from "./parts";
