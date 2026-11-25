/** Part contracts: one typed schema per storefront part yields its React component, element, builder form and server checks. */
import { z } from "zod";
import type { ComponentType, ReactNode } from "react";

export type PartEvents = {
  /** A buyer changed a cart line's quantity (0 removes it); the server reprices. */
  onQuantity?: (detail: {
    lineId: string;
    quantity: number;
    /** The buyer left the quantity field; hosts may snap to purchase steps. */
    settled?: boolean;
  }) => void;
  /** A buyer chose a variant option. */
  onSelect?: (detail: { group: string; value: string }) => void;
  /** A buyer opened a line, order or link inside the part. */
  onOpen?: (detail: { kind: "line" | "order" | "page"; id: string }) => void;
  /**
   * Host-rendered additions that are not data (app configuration summaries,
   * coupon forms). Web elements and MCP hosts leave them out.
   */
  renderLineDetail?: (lineId: string) => ReactNode;
  slot?: (
    name: "beforeTotals" | "afterTotals" | "end" | "beforeBuy",
  ) => ReactNode;
  /** A buyer asked to add a product; hosts decide how (cart API, chat, app). */
  onAddToCart?: (detail: { productId: string; quantity: number }) => void;
  /** A buyer asked to continue to checkout. */
  onCheckout?: () => void;
};

export type PartDefinition<
  S extends z.ZodObject,
  C extends z.ZodObject = z.ZodObject,
> = {
  /** Stable id; also the element name `veyndo-<id>` and the builder block id. */
  id: string;
  /** Raised when the props change incompatibly; stored blocks keep their version. */
  version: number;
  /** The props the part shows: data the host resolved from the shop. */
  schema: S;
  /**
   * What a merchant sets in the Visual Builder (for example which product);
   * the host resolves it with live shop data into props.
   */
  config: C;
  Component: ComponentType<z.infer<S> & PartEvents>;
};

export type Part<
  S extends z.ZodObject = z.ZodObject,
  C extends z.ZodObject = z.ZodObject,
> = PartDefinition<S, C> & {
  element: string;
  /** Validates untrusted props (attributes, stored blocks, model output). */
  parse: (
    value: unknown,
  ) => { ok: true; props: z.infer<S> } | { ok: false; issues: string[] };
  /** Validates a stored or proposed builder configuration. */
  parseConfig: (
    value: unknown,
  ) => { ok: true; config: z.infer<C> } | { ok: false; issues: string[] };
  /** JSON Schema of the props, for MCP tools and the server. */
  jsonSchema: () => Record<string, unknown>;
  /** JSON Schema of the builder configuration, for builder forms and the server. */
  configSchema: () => Record<string, unknown>;
};

/** A part whose prop types are erased, for hosts that handle every part alike (elements, forms, registries). */
export type AnyPart = Omit<Part, "Component"> & {
  // Props are validated at runtime by `parse` before they reach the component.
  Component: ComponentType<any>;
};

const check = <T extends z.ZodType>(schema: T, value: unknown) => {
  const result = schema.safeParse(value);
  return result.success
    ? ({ ok: true, value: result.data as z.infer<T> } as const)
    : ({
        ok: false,
        issues: result.error.issues.map(
          (issue) => `${issue.path.join(".") || "value"}: ${issue.message}`,
        ),
      } as const);
};

export function definePart<S extends z.ZodObject, C extends z.ZodObject>(
  definition: PartDefinition<S, C>,
): Part<S, C> {
  if (!/^[a-z][a-z0-9-]*$/.test(definition.id))
    throw Error(`Invalid part id ${definition.id}`);
  return {
    ...definition,
    element: `veyndo-${definition.id}`,
    parse: (value) => {
      const result = check(definition.schema, value);
      return result.ok
        ? { ok: true, props: result.value }
        : { ok: false, issues: result.issues };
    },
    parseConfig: (value) => {
      const result = check(definition.config, value);
      return result.ok
        ? { ok: true, config: result.value }
        : { ok: false, issues: result.issues };
    },
    jsonSchema: () =>
      z.toJSONSchema(definition.schema, { target: "draft-2020-12" }) as Record<
        string,
        unknown
      >,
    configSchema: () =>
      z.toJSONSchema(definition.config, {
        target: "draft-2020-12",
        io: "input",
      }) as Record<string, unknown>,
  };
}

/** Shared field shapes of the part contracts. */
export const locale = z.string().min(2).max(35).default("en-GB");
export const currency = z.string().regex(/^[A-Z]{3}$/);
export const amount = z.number().finite();
/** A product of this shop, chosen in the builder; hosts render a product picker for it. */
export const productId = z
  .string()
  .min(1)
  .max(200)
  .meta({ format: "product-id" });
/** A relative or https URL; the pattern also travels in the JSON Schema. */
export const url = z
  .string()
  .max(2000)
  .regex(/^(\/(?!\/)|https:\/\/)/, "Only relative or https URLs");
