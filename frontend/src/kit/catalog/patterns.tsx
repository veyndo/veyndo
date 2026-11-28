/** Composed patterns: the storefront parts as catalog entries; commerce data must come bound from host-admitted data. */
import { z } from "zod";
import {
  accountOverview,
  cartSummary,
  checkoutSteps,
  orderList,
  productCard,
  productSummary,
  type AnyPart,
} from "../storefront";
import { defineEntry, type CatalogEntry, type Surface } from "./define";
import { identifier } from "./fields";

const opened = z.object({
  kind: z.enum(["line", "order", "page"]),
  id: z.string().max(200),
});

type PatternSpec = {
  examples?: CatalogEntry["examples"];
  name: string;
  part: AnyPart;
  description: string;
  hostData: string[];
  events: CatalogEntry["events"];
  surfaces: Surface[];
  a11y: CatalogEntry["a11y"];
};

/** A part's props without `locale`: the host supplies the language, not the document. */
const documentProps = (part: AnyPart) =>
  (part.schema as z.ZodObject).omit({ locale: true } as never).strict();

function pattern(spec: PatternSpec): CatalogEntry {
  const Component = spec.part.Component;
  return defineEntry({
    name: spec.name,
    kind: "pattern",
    description: spec.description,
    source: `kit/storefront#${spec.part.id}`,
    since: "1.0.0",
    props: documentProps(spec.part),
    hostData: spec.hostData,
    events: spec.events,
    a11y: spec.a11y,
    surfaces: spec.surfaces,
    render: ({ props, emit, bound, locale }) => (
      <Component
        {...props}
        locale={locale}
        onAddToCart={
          bound("addToCart")
            ? (detail: unknown) => emit("addToCart", detail)
            : undefined
        }
        onCheckout={bound("checkout") ? () => emit("checkout") : undefined}
        onOpen={
          bound("open") ? (detail: unknown) => emit("open", detail) : undefined
        }
        onQuantity={
          bound("quantity")
            ? (detail: unknown) => emit("quantity", detail)
            : undefined
        }
        onSelect={
          bound("select")
            ? (detail: unknown) => emit("select", detail)
            : undefined
        }
      />
    ),
    examples: spec.examples ?? [],
  });
}

const bag = (t: (key: string) => string) => ({
  id: "bag",
  name: t("bag"),
  url: "/products/bag",
  category: t("bagCategory"),
  price: 169,
  currency: "EUR",
  stock: 4,
});

export const patternEntries: CatalogEntry[] = [
  pattern({
    name: "ProductCard",
    part: productCard,
    description:
      "A product tile with image, name, price, tax note and stock from the shop.",
    hostData: ["product"],
    events: {},
    surfaces: ["studio", "storefront", "mcp-app"],
    examples: [
      {
        id: "card",
        state: "in stock",
        data: (t) => ({ product: bag(t) }),
        node: () => ({
          component: "ProductCard",
          props: { product: { path: "/product" } },
        }),
      },
    ],
    a11y: {
      role: "article",
      name: "content",
      notes:
        "The name links to the product page; prices come only from the host.",
    },
  }),
  pattern({
    name: "ProductSummary",
    part: productSummary,
    description:
      "The purchase area of a product: variants, price, stock and add to cart.",
    hostData: ["product", "groups", "tiers", "delivery"],
    events: {
      addToCart: {
        description: "The buyer asked to add the product.",
        payload: z.object({
          productId: identifier,
          quantity: z.number().int().min(1),
        }),
      },
      select: {
        description: "The buyer chose a variant option.",
        payload: z.object({ group: z.string(), value: z.string() }),
      },
      open: {
        description: "The buyer opened reviews or variants.",
        payload: opened,
      },
    },
    surfaces: ["storefront", "mcp-app"],
    a11y: {
      name: "content",
      notes:
        "Add to cart is a host action; the document never confirms a purchase.",
    },
  }),
  pattern({
    name: "CartSummary",
    part: cartSummary,
    description:
      "Priced cart lines, discounts, taxes and totals as the server calculated them.",
    hostData: [
      "currency",
      "lines",
      "subtotal",
      "discounts",
      "shipping",
      "tax",
      "total",
      "delivery",
    ],
    examples: [
      {
        id: "cart",
        state: "one line",
        actions: ["cart"],
        data: (t) => ({
          cart: {
            currency: "EUR",
            lines: [
              {
                id: "l1",
                label: t("bag"),
                quantity: 1,
                unitPrice: 169,
                total: 169,
                url: "/products/bag",
              },
            ],
            subtotal: 169,
            shipping: 4.9,
            total: 173.9,
          },
        }),
        node: () => ({
          component: "CartSummary",
          props: {
            currency: { path: "/cart/currency" },
            lines: { path: "/cart/lines" },
            subtotal: { path: "/cart/subtotal" },
            shipping: { path: "/cart/shipping" },
            total: { path: "/cart/total" },
          },
          action: "cart",
        }),
      },
    ],
    events: {
      quantity: {
        description: "The buyer changed a line quantity; the server reprices.",
        payload: z.object({
          lineId: z.string(),
          quantity: z.number().int().min(0),
        }),
      },
      checkout: { description: "The buyer asked to continue to checkout." },
      open: { description: "The buyer opened a line.", payload: opened },
    },
    surfaces: ["storefront", "mcp-app"],
    a11y: {
      role: "region",
      name: "content",
      notes:
        "Totals are announced as text; quantity fields are labelled per line.",
    },
  }),
  pattern({
    name: "CheckoutSteps",
    part: checkoutSteps,
    description:
      "Where the buyer is in checkout; it never implies a confirmed payment.",
    hostData: [],
    events: {},
    examples: [
      {
        id: "review",
        state: "review",
        node: () => ({
          component: "CheckoutSteps",
          props: { current: "review" },
        }),
      },
    ],
    surfaces: ["storefront", "mcp-app"],
    a11y: {
      role: "list",
      name: "content",
      notes: "The current step carries aria-current=step.",
    },
  }),
  pattern({
    name: "AccountOverview",
    part: accountOverview,
    description:
      "The account home: greeting, counts and recent orders of the signed-in customer.",
    hostData: ["name", "counts", "orders"],
    events: {
      open: {
        description: "The customer opened an order or account page.",
        payload: opened,
      },
    },
    surfaces: ["storefront"],
    a11y: {
      role: "region",
      name: "content",
      notes: "Personal data; only the storefront account host admits it.",
    },
  }),
  pattern({
    name: "OrderList",
    part: orderList,
    description: "The signed-in customer's orders.",
    hostData: ["orders"],
    events: {
      open: { description: "The customer opened an order.", payload: opened },
    },
    surfaces: ["storefront"],
    a11y: {
      role: "list",
      name: "content",
      notes: "Personal data; only the storefront account host admits it.",
    },
  }),
];
