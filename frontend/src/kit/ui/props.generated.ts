/** Generated from kit/catalog by `WRITE_COMPONENT_CATALOG=1 npm test -- component-catalog`; do not edit. Document prop types of catalog veyndo:components/1 (1.0.0). */
export const catalogId = "veyndo:components/1";
export const catalogVersion = "1.0.0";

/** A paragraph of plain text. */
export type TextDocumentProps = {
  text: string;
  tone?: "default" | "muted";
};

/** A section heading at level 2, 3 or 4, so the document fits the host's outline. */
export type HeadingDocumentProps = {
  text: string;
  level?: 2 | 3 | 4;
};

/** An image from a relative or https URL on an admitted origin, with required alternative text. */
export type ImageDocumentProps = {
  src: string;
  alt: string;
  decorative?: boolean;
  width?: number;
  height?: number;
};

/** Arranges children in a row or column on the spacing scale. */
export type StackDocumentProps = {
  gap?: "1" | "2" | "3" | "4" | "5" | "6" | "8" | "10";
  direction?: "row" | "column";
  align?: "start" | "center" | "end" | "stretch";
  wrap?: boolean;
};

/** A titled surface that groups related content, with optional header actions. */
export type CardDocumentProps = {
  title?: string;
  level?: 2 | 3 | 4;
};

/** Triggers an admitted host action. Without an admitted action it renders disabled. */
export type ButtonDocumentProps = {
  label: string;
  tone?: "primary" | "secondary" | "ghost" | "danger";
  size?: "sm" | "md";
  busy?: boolean;
  disabled?: boolean;
};

/** A button with only an icon and a required accessible label. */
export type IconButtonDocumentProps = {
  label: string;
  tone?: "primary" | "secondary" | "ghost" | "danger";
  size?: "sm" | "md";
};

/** Navigation that looks like a button; keeps link semantics. URLs must be relative or on an admitted https origin. */
export type LinkButtonDocumentProps = {
  label: string;
  href: string;
  tone?: "primary" | "secondary" | "ghost" | "danger";
  size?: "sm" | "md";
};

/** A short status label. */
export type BadgeDocumentProps = {
  text: string;
  tone?: "neutral" | "accent" | "positive" | "warning" | "danger";
};

/** Progress of unknown length. With a label it is a polite status; without one it is decorative. */
export type SpinnerDocumentProps = {
  label?: string;
};

/** Text only for assistive technology. */
export type VisuallyHiddenDocumentProps = {
  text: string;
};

/** Binds a label, hint and error to one control. */
export type FieldDocumentProps = {
  label: string;
  hint?: string;
  error?: string;
  required?: boolean;
};

/** A labelled single-line text field. The value stays in the view until committed. */
export type TextInputDocumentProps = {
  name: string;
  label: string;
  hint?: string;
  error?: string;
  required?: boolean;
  value?: string;
  placeholder?: string;
  type?: "text" | "email" | "number" | "search" | "tel" | "url";
};

/** A labelled multi-line text field. */
export type TextAreaDocumentProps = {
  name: string;
  label: string;
  hint?: string;
  error?: string;
  required?: boolean;
  value?: string;
  placeholder?: string;
  rows?: number;
};

/** A labelled choice from a short list. */
export type SelectDocumentProps = {
  name: string;
  label: string;
  hint?: string;
  options: {
    value: string;
    label: string;
  }[];
  value?: string;
};

/** A labelled checkbox. */
export type CheckboxDocumentProps = {
  name: string;
  label: string;
  checked?: boolean;
};

/** An on/off switch for a setting that applies at once. */
export type SwitchDocumentProps = {
  name: string;
  label: string;
  checked?: boolean;
  disabled?: boolean;
};

/** A message about the current state. Danger is announced at once; other tones are polite. */
export type NoticeDocumentProps = {
  tone?: "neutral" | "accent" | "positive" | "warning" | "danger";
  title?: string;
  text?: string;
};

/** Explains why a list or area is empty and what to do next. */
export type EmptyStateDocumentProps = {
  title: string;
  text?: string;
};

/** Switches between panels of related content. */
export type TabsDocumentProps = {
  label: string;
  orientation?: "horizontal" | "vertical";
  value?: string;
};

/** One tab and its panel inside Tabs. */
export type TabPanelDocumentProps = {
  id: string;
  label: string;
  disabled?: boolean;
};

/** A button that opens a list of actions. */
export type MenuDocumentProps = {
  label: string;
  items: {
    id: string;
    label: string;
    disabled?: boolean;
    tone?: "danger";
  }[];
};

/** A modal dialog with focus containment and restoration. */
export type DialogDocumentProps = {
  title: string;
  description?: string;
  dismissible?: boolean;
};

/** A product tile with image, name, price, tax note and stock from the shop. */
export type ProductCardDocumentProps = {
  product: {
    id: string;
    name: string;
    url: string;
    image?: string;
    description?: string;
    category?: string;
    price: number;
    currency: string;
    stock?: number;
  };
  priceKind?: "gross" | "net";
};

/** The purchase area of a product: variants, price, stock and add to cart. */
export type ProductSummaryDocumentProps = {
  product: {
    id: string;
    name: string;
    description?: string;
    image?: string;
    price: number;
    listPrice?: number;
    currency: string;
    stock: number;
    deliveryDays?: number;
    minPurchase?: number;
    purchaseSteps?: number;
    maxPurchase?: number;
    kicker?: string;
    sku?: string;
    rating?: {
      average: number;
      count: number;
    };
    reference?: {
      price: number;
      unit: string;
    };
  };
  priceKind?: "gross" | "net";
  shippingExtra?: boolean;
  groups?: {
    id: string;
    label: string;
    selected: string;
    options: {
      value: string;
      label: string;
      available: boolean;
      selected: boolean;
      swatch?: string;
    }[];
  }[];
  moreVariants?: "none" | "more" | "loading";
  showRules?: boolean;
  tiers?: {
    quantity: number;
    unitPrice: number;
    discountPercent: number;
  }[];
  selectedTier?: number;
  delivery?: {
    minDays: number;
    maxDays: number;
    method: string;
    country: string;
  };
  quantity?: number;
  disabled?: boolean;
};

/** Priced cart lines, discounts, taxes and totals as the server calculated them. */
export type CartSummaryDocumentProps = {
  currency: string;
  lines: {
    id: string;
    label: string;
    quantity: number;
    unitPrice?: number;
    total: number;
    url?: string;
    image?: string;
    minPurchase?: number;
    purchaseSteps?: number;
    maxPurchase?: number;
  }[];
  subtotal?: number;
  discounts?: {
    id: string;
    label: string;
    amount: number;
  }[];
  shipping?: number;
  tax?: number;
  total: number;
  delivery?: {
    earliest: string;
    latest: string;
  };
  priceKind?: "gross" | "net";
  busy?: boolean;
};

/** Where the buyer is in checkout; it never implies a confirmed payment. */
export type CheckoutStepsDocumentProps = {
  current?: "details" | "review" | "complete";
};

/** The account home: greeting, counts and recent orders of the signed-in customer. */
export type AccountOverviewDocumentProps = {
  name: string;
  counts: {
    orders: number;
    addresses: number;
    downloads: number;
  };
  orders: {
    id: string;
    number: string;
    date?: string;
    state: string;
    total: number;
    currency: string;
    items?: {
      label: string;
      quantity: number;
      image?: string;
    }[];
  }[];
  recent?: number;
};

/** The signed-in customer's orders. */
export type OrderListDocumentProps = {
  orders: {
    id: string;
    number: string;
    date?: string;
    state: string;
    total: number;
    currency: string;
    items?: {
      label: string;
      quantity: number;
      image?: string;
    }[];
  }[];
};

/** An action a document binds to an interactive component; the host admits its name. */
export type DocumentAction =
  | string
  | { name: string; context?: Record<string, string | number | boolean> };
/** A value bound to host-admitted data by JSON Pointer. */
export type DocumentBinding = { path: string };
type Bindable<P> = { [K in keyof P]: P[K] | DocumentBinding };
/** One node of a `veyndo-ui/1` document. */
export type DocumentNode =
  | { component: "Text"; props?: Bindable<TextDocumentProps> }
  | { component: "Heading"; props?: Bindable<HeadingDocumentProps> }
  | { component: "Image"; props?: Bindable<ImageDocumentProps> }
  | {
      component: "Stack";
      props?: Bindable<StackDocumentProps>;
      children?: DocumentNode[];
    }
  | {
      component: "Card";
      props?: Bindable<CardDocumentProps>;
      children?: DocumentNode[];
      slots?: { actions?: DocumentNode[] };
    }
  | {
      component: "Button";
      props?: Bindable<ButtonDocumentProps>;
      action?: DocumentAction;
    }
  | { component: "LinkButton"; props?: Bindable<LinkButtonDocumentProps> }
  | { component: "Badge"; props?: Bindable<BadgeDocumentProps> }
  | { component: "Spinner"; props?: Bindable<SpinnerDocumentProps> }
  | {
      component: "VisuallyHidden";
      props?: Bindable<VisuallyHiddenDocumentProps>;
    }
  | {
      component: "TextInput";
      props?: Bindable<TextInputDocumentProps>;
      action?: DocumentAction;
    }
  | {
      component: "TextArea";
      props?: Bindable<TextAreaDocumentProps>;
      action?: DocumentAction;
    }
  | {
      component: "Select";
      props?: Bindable<SelectDocumentProps>;
      action?: DocumentAction;
    }
  | {
      component: "Checkbox";
      props?: Bindable<CheckboxDocumentProps>;
      action?: DocumentAction;
    }
  | {
      component: "Switch";
      props?: Bindable<SwitchDocumentProps>;
      action?: DocumentAction;
    }
  | {
      component: "Notice";
      props?: Bindable<NoticeDocumentProps>;
      slots?: { action?: DocumentNode[] };
    }
  | {
      component: "EmptyState";
      props?: Bindable<EmptyStateDocumentProps>;
      slots?: { action?: DocumentNode[] };
    }
  | {
      component: "Tabs";
      props?: Bindable<TabsDocumentProps>;
      action?: DocumentAction;
      children?: DocumentNode[];
    }
  | {
      component: "TabPanel";
      props?: Bindable<TabPanelDocumentProps>;
      children?: DocumentNode[];
    }
  | {
      component: "Menu";
      props?: Bindable<MenuDocumentProps>;
      action?: DocumentAction;
    }
  | { component: "ProductCard"; props?: Bindable<ProductCardDocumentProps> }
  | {
      component: "ProductSummary";
      props?: Bindable<ProductSummaryDocumentProps>;
      action?: DocumentAction;
    }
  | {
      component: "CartSummary";
      props?: Bindable<CartSummaryDocumentProps>;
      action?: DocumentAction;
    }
  | { component: "CheckoutSteps"; props?: Bindable<CheckoutStepsDocumentProps> }
  | {
      component: "AccountOverview";
      props?: Bindable<AccountOverviewDocumentProps>;
      action?: DocumentAction;
    }
  | {
      component: "OrderList";
      props?: Bindable<OrderListDocumentProps>;
      action?: DocumentAction;
    };
/** A complete `veyndo-ui/1` document. */
export type UiDocument = {
  format: "veyndo-ui/1";
  catalog: typeof catalogId;
  root: DocumentNode;
};
