/** Reads `veyndo-ui/1`, A2UI v1.0 and OpenUI Lang documents into one untrusted node tree; nothing here is validated yet. */
import { createParser } from "@openuidev/react-lang";
import { catalogId, entries, limits } from "./catalog";
import { openUiArguments } from "./define";

/** A node as the document stated it: component name, raw props, action and slot children. */
export type RawNode = {
  id: string;
  component: unknown;
  props: Record<string, unknown>;
  action?: unknown;
  slots: Record<string, RawNode[]>;
};

export type Format = "veyndo-ui/1" | "a2ui/1.0" | "openui-lang";

export type ReadResult =
  | { ok: true; format: Format; catalogId: string; root: RawNode }
  | {
      ok: false;
      code: "format" | "catalog" | "limits" | "data-not-admitted";
      message: string;
    };

const isRecord = (value: unknown): value is Record<string, unknown> =>
  typeof value === "object" && value !== null && !Array.isArray(value);

const fail = (
  code: Exclude<ReadResult, { ok: true }>["code"],
  message: string,
) => ({ ok: false, code, message }) as const;

/** Host-owned components a host registers next to the catalog, by name with their slot names. */
export type HostShapes = Readonly<
  Record<string, { slots?: Readonly<Record<string, unknown>> }>
>;
let host: HostShapes = {};

/** Slot names of an entry or host component; `children` is always the default slot. */
const slotNames = (component: unknown) => {
  const entry = entries.find((candidate) => candidate.name === component);
  const own =
    typeof component === "string" && Object.hasOwn(host, component)
      ? host[component].slots
      : undefined;
  return new Set(["children", ...Object.keys(entry?.slots ?? own ?? {})]);
};

class Budget {
  nodes = 0;
  take(depth: number) {
    this.nodes += 1;
    if (this.nodes > limits.nodes)
      throw new LimitError(`more than ${limits.nodes} nodes`);
    if (depth > limits.depth)
      throw new LimitError(`deeper than ${limits.depth} levels`);
  }
}
class LimitError extends Error {}
class ShapeError extends Error {}

function readTreeNode(
  value: unknown,
  id: string,
  depth: number,
  budget: Budget,
): RawNode {
  budget.take(depth);
  if (!isRecord(value)) throw new ShapeError(`${id}: a node must be an object`);
  const {
    component,
    props = {},
    action,
    children,
    slots = {},
    ...rest
  } = value;
  if (Object.keys(rest).length)
    throw new ShapeError(
      `${id}: unknown node keys ${Object.keys(rest).join(", ")}`,
    );
  if (!isRecord(props) || !isRecord(slots))
    throw new ShapeError(`${id}: props and slots must be objects`);
  const read = (list: unknown, slot: string) => {
    if (!Array.isArray(list))
      throw new ShapeError(`${id}: slot ${slot} must be a list`);
    return list.map((child, index) =>
      readTreeNode(child, `${id}.${slot}.${index}`, depth + 1, budget),
    );
  };
  const out: Record<string, RawNode[]> = {};
  if (children !== undefined) out.children = read(children, "children");
  for (const [slot, list] of Object.entries(slots))
    out[slot] = read(list, slot);
  return { id, component, props, action, slots: out };
}

function readTree(value: Record<string, unknown>): ReadResult {
  if (typeof value.catalog !== "string")
    return fail("catalog", "the document names no catalog");
  if ("data" in value)
    return fail(
      "data-not-admitted",
      "documents cannot supply data; hosts admit it",
    );
  return {
    ok: true,
    format: "veyndo-ui/1",
    catalogId: value.catalog,
    root: readTreeNode(value.root, "root", 1, new Budget()),
  };
}

type A2uiComponent = Record<string, unknown> & {
  id: string;
  component: unknown;
};

/** A2UI v1.0: `createSurface` names the catalog, `updateComponents` lists flat components that refer to each other by id. */
function readA2ui(messages: unknown[]): ReadResult {
  let catalog: string | undefined;
  const components = new Map<string, A2uiComponent>();
  for (const message of messages) {
    if (!isRecord(message) || message.version !== "1.0")
      throw new ShapeError("every A2UI message must have version 1.0");
    if (isRecord(message.createSurface)) {
      if (typeof message.createSurface.catalogId !== "string")
        throw new ShapeError("createSurface needs a catalogId");
      catalog = message.createSurface.catalogId;
    } else if (isRecord(message.updateComponents)) {
      const list = message.updateComponents.components;
      if (!Array.isArray(list))
        throw new ShapeError("updateComponents needs components");
      for (const component of list) {
        if (!isRecord(component) || typeof component.id !== "string")
          throw new ShapeError("every component needs an id");
        components.set(component.id, component as A2uiComponent);
      }
    } else if ("updateDataModel" in message) {
      return fail(
        "data-not-admitted",
        "documents cannot supply a data model; hosts admit data",
      );
    } else if (!("deleteSurface" in message)) {
      throw new ShapeError(
        `unsupported A2UI message ${Object.keys(message).join(", ")}`,
      );
    }
  }
  if (!catalog) return fail("catalog", "no createSurface names a catalog");
  if (components.size > limits.nodes)
    throw new LimitError(`more than ${limits.nodes} nodes`);
  const budget = new Budget();
  const used = new Set<string>();
  const build = (id: string, depth: number): RawNode => {
    const component = components.get(id);
    if (!component)
      throw new ShapeError(`component ${id} is referenced but not defined`);
    if (used.has(id))
      throw new ShapeError(`component ${id} appears twice or in a cycle`);
    used.add(id);
    budget.take(depth);
    const slots = slotNames(component.component);
    const props: Record<string, unknown> = {};
    const out: Record<string, RawNode[]> = {};
    let action: unknown;
    for (const [key, value] of Object.entries(component)) {
      if (key === "id" || key === "component") continue;
      if (key === "action") action = value;
      else if (
        key === "child" ||
        key === "children" ||
        (slots.has(key) && (typeof value === "string" || Array.isArray(value)))
      ) {
        const ids =
          key === "child" ? [value] : Array.isArray(value) ? value : [value];
        if (!ids.every((ref) => typeof ref === "string"))
          throw new ShapeError(`${id}: ${key} must name component ids`);
        out[key === "child" ? "children" : key] = (ids as string[]).map((ref) =>
          build(ref, depth + 1),
        );
      } else props[key] = value;
    }
    return { id, component: component.component, props, action, slots: out };
  };
  return {
    ok: true,
    format: "a2ui/1.0",
    catalogId: catalog,
    root: build("root", 1),
  };
}

type OpenUiElement = {
  type: "element";
  typeName: string;
  props: Record<string, unknown>;
  partial?: boolean;
};
const isElement = (value: unknown): value is OpenUiElement =>
  isRecord(value) && value.type === "element";

/** The OpenUI library schema of the catalog and the host's components, with positional arguments as `openUiArguments` orders them. */
const openUiSchema = () => ({
  $defs: Object.fromEntries([
    ...Object.entries(host).map(([name, shape]) => [
      name,
      {
        type: "object",
        properties: Object.fromEntries(
          ["children", ...Object.keys(shape.slots ?? {})].map((slot) => [
            slot,
            {},
          ]),
        ),
        required: [],
      },
    ]),
    ...entries
      .filter((entry) => entry.surfaces.length)
      .map((entry) => [
        entry.name,
        {
          type: "object",
          properties: Object.fromEntries(
            openUiArguments(entry).map((key) => [key, {}]),
          ),
          required: [],
        },
      ]),
  ]),
});
const parsers = new WeakMap<HostShapes, ReturnType<typeof createParser>>();

function readOpenUi(source: string, catalog: string): ReadResult {
  const parser = parsers.get(host) ?? createParser(openUiSchema());
  parsers.set(host, parser);
  const result = parser.parse(source);
  if (result.meta.incomplete || !result.root)
    throw new ShapeError("the OpenUI document is incomplete");
  if (result.meta.unresolved.length)
    throw new ShapeError(
      `unresolved references ${result.meta.unresolved.join(", ")}`,
    );
  if (result.meta.errors.length)
    throw new ShapeError(
      result.meta.errors.map((error) => error.message).join("; "),
    );
  const budget = new Budget();
  const convert = (element: unknown, id: string, depth: number): RawNode => {
    budget.take(depth);
    if (!isElement(element))
      throw new ShapeError(`${id}: expected a component`);
    const slots = slotNames(element.typeName);
    const props: Record<string, unknown> = {};
    const out: Record<string, RawNode[]> = {};
    let action: unknown;
    for (const [key, value] of Object.entries(element.props)) {
      if (value === null || value === undefined) continue;
      if (key === "action") action = value;
      else if (
        slots.has(key) &&
        (isElement(value) || (Array.isArray(value) && value.every(isElement)))
      )
        out[key] = (Array.isArray(value) ? value : [value]).map(
          (child, index) => convert(child, `${id}.${key}.${index}`, depth + 1),
        );
      else props[key] = value;
    }
    return { id, component: element.typeName, props, action, slots: out };
  };
  return {
    ok: true,
    format: "openui-lang",
    catalogId: catalog,
    root: convert(result.root, "root", 1),
  };
}

const none: HostShapes = {};

/**
 * Reads a document in any supported format. OpenUI Lang carries no catalog id of its
 * own; the host states the one its prompt used (`openUiCatalog`, default this catalog).
 */
export function readDocument(
  input: unknown,
  options: { openUiCatalog?: string; host?: HostShapes } = {},
): ReadResult {
  host = options.host ?? none;
  const openUiCatalog = options.openUiCatalog ?? catalogId;
  try {
    const size =
      typeof input === "string"
        ? input.length
        : JSON.stringify(input ?? null).length;
    if (size > limits.bytes)
      return fail("limits", `larger than ${limits.bytes} bytes`);
    if (typeof input === "string") {
      const text = input.trim();
      if (text.startsWith("{") || text.startsWith("["))
        return readDocument(JSON.parse(text), options);
      return readOpenUi(text, openUiCatalog);
    }
    if (Array.isArray(input)) return readA2ui(input);
    if (isRecord(input) && Array.isArray(input.messages))
      return readA2ui(input.messages);
    if (isRecord(input) && input.format === "veyndo-ui/1")
      return readTree(input);
    return fail(
      "format",
      "not a veyndo-ui/1, A2UI v1.0 or OpenUI Lang document",
    );
  } catch (error) {
    if (error instanceof LimitError) return fail("limits", error.message);
    return fail(
      "format",
      error instanceof Error ? error.message : "unreadable document",
    );
  }
}
