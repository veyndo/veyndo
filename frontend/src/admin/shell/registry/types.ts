/** Types of the Studio extension registry: routes, navigation, page slots, dashboard cards and form fields. */
import type { ComponentType, LazyExoticComponent } from "react";
import type { IconName } from "../../../shared/ui/Icon";
import type { OrderPaymentContext } from "../../../../../components/payments/ui/admin/payments/payment-amount";

/** Presentation admission: every `all` right and at least one `any` right. The server decides again. */
export type Rights = { any: string[]; all?: string[] };

export type NavigationGroup =
  "intelligence" | "commerce" | "experiences" | "workspace";

/** What a route needs before it can render; the shell shows its own empty state otherwise. */
export type RouteRequirement = "connection" | "data";

export type StudioRoute<Id extends string = string> = {
  id: Id;
  rights: Rights;
  icon: IconName;
  /** Listed in this sidebar group; routes without a group are reached otherwise. */
  group?: NavigationGroup;
  /** Order within the sidebar group, lower first; equal positions keep registration order. */
  position?: number;
  requires?: RouteRequirement;
  /** Code-split view; it reads the Studio context itself. */
  component: LazyExoticComponent<ComponentType>;
};

/**
 * Slot locations shared with installed apps (`admin.order`, `admin.order.general`,
 * `admin.product.general`) and Studio's own dashboard. The value is the context a
 * contribution receives.
 */
export type SlotContexts = {
  "admin.dashboard": Record<string, never>;
  "admin.customer": { customerId: string };
  "admin.order": { orderId: string };
  "admin.order.general": { orderId: string };
  /** The order page hosts the payment ledger of the payments component here. */
  "admin.order.payment": OrderPaymentContext;
  "admin.product.general": { productId: string };
};
export type SlotLocation = keyof SlotContexts;

/** The kinds of contribution; each kind may only use its own locations. */
export type ContributionKind = "page-slot" | "dashboard-card" | "form-field";
export const contributionLocations: Record<ContributionKind, SlotLocation[]> = {
  "page-slot": [
    "admin.customer",
    "admin.order",
    "admin.order.general",
    "admin.order.payment",
  ],
  "dashboard-card": ["admin.dashboard"],
  "form-field": ["admin.product.general"],
};

export type Contribution<L extends SlotLocation = SlotLocation> = {
  id: string;
  kind: ContributionKind;
  location: L;
  rights?: Rights;
  /** Lower first; equal orders keep registration order. */
  order?: number;
  component: LazyExoticComponent<ComponentType<{ context: SlotContexts[L] }>>;
};

/** One first-party component's contributions, declared like an app's manifest. */
export type StudioExtension = {
  id: string;
  routes?: StudioRoute[];
  contributions?: Contribution[];
};
