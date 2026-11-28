/** The shape of one catalog entry: props schema, slots, events, accessibility contract, surfaces, examples and renderer. */
import type { ReactNode } from "react";
import type { z } from "zod";

/** Where a document is rendered. Each surface admits its own subset of the catalog. */
export const surfaces = ["studio", "storefront", "app", "mcp-app"] as const;
export type Surface = (typeof surfaces)[number];

export type Kind = "content" | "layout" | "atom" | "molecule" | "pattern";

export type SlotRule = {
  description: string;
  /** Component names a slot accepts; `"inline"` accepts every entry of kind content, atom or molecule. */
  accepts: readonly string[] | "inline" | "any";
  min?: number;
  max?: number;
};

export type EventRule = {
  description: string;
  /** Payload the component reports with the event; the host receives it with the document's action. */
  payload?: z.ZodType;
};

export type Accessibility = {
  /** The WAI-ARIA role the rendered element carries, if any. */
  role?: string;
  /** The WAI-ARIA authoring pattern the component follows. */
  pattern?: string;
  /** Where the accessible name comes from: a named prop, the content, or none needed. */
  name: { prop: string } | "content" | "none";
  keyboard?: readonly string[];
  notes: string;
};

/** Sample text keys an example uses; the gallery resolves them in the selected language. */
export type Sample = (key: string) => string;

export type ExampleNode = {
  component: string;
  props?: Record<string, unknown>;
  action?: unknown;
  slots?: Record<string, ExampleNode[]>;
  children?: ExampleNode[];
};

export type Example = {
  id: string;
  /** The state the example shows, for example `default`, `busy`, `invalid`. */
  state: string;
  node: (t: Sample) => ExampleNode;
  /** Host data the example binds to, as a host would admit it. */
  data?: (t: Sample) => Record<string, unknown>;
  /** Action names the example's host admits. */
  actions?: readonly string[];
};

export type RenderContext<P> = {
  props: P;
  slots: Record<string, ReactNode[]>;
  /** The validated props of each slot's children, for containers that lay them out themselves (Tabs). */
  slotProps: Record<string, Record<string, unknown>[]>;
  /** Reports an event; the renderer forwards it to the host only when the document bound an admitted action. */
  emit: (event: string, payload?: unknown) => void;
  /** Whether the document bound an action to `event`; unbound controls render disabled. */
  bound: (event: string) => boolean;
  locale: string;
  id: string;
};

export type CatalogEntry<S extends z.ZodObject = z.ZodObject> = {
  name: string;
  kind: Kind;
  description: string;
  /** The kit export this entry renders, for documentation. */
  source: string;
  /** Catalog version in which the entry appeared. */
  since: string;
  props: S;
  /** Props that must be bound to data the host admitted; literal values are refused. */
  hostData?: readonly string[];
  slots?: Record<string, SlotRule>;
  events?: Record<string, EventRule>;
  a11y: Accessibility;
  /** Parents the entry may appear in; omitted means anywhere a slot accepts it. */
  within?: readonly string[];
  /** Surfaces whose documents may use the entry; empty means gallery and code only. */
  surfaces: readonly Surface[];
  /** Why a code-only entry is not admitted in documents. */
  codeOnly?: string;
  render: (context: RenderContext<z.infer<S>>) => ReactNode;
  examples: readonly Example[];
};

/** OpenUI Lang's positional arguments of an entry: the default slot first, then props in order, other slots, then `action`. */
export const openUiArguments = (entry: CatalogEntry) => [
  ...(entry.slots?.children ? ["children"] : []),
  ...Object.keys(entry.props.shape),
  ...Object.keys(entry.slots ?? {}).filter((slot) => slot !== "children"),
  ...(entry.events && Object.keys(entry.events).length ? ["action"] : []),
];

export function defineEntry<S extends z.ZodObject>(
  entry: CatalogEntry<S>,
): CatalogEntry {
  if (!/^[A-Z][A-Za-z0-9]*$/.test(entry.name))
    throw Error(`Invalid catalog entry name ${entry.name}`);
  return entry as unknown as CatalogEntry;
}
