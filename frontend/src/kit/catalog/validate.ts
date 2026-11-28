/** Validates a read document against the catalog and the host's admission: components, props, URLs, data, actions and slots. */
import type { ReactNode } from "react";
import type { z } from "zod";
import { catalogId, entryByName } from "./catalog";
import type { CatalogEntry, SlotRule, Surface } from "./define";
import { action as actionField, safeUrl } from "./fields";
import { readDocument, type Format, type RawNode } from "./document";

/** A host-owned component, such as a native Studio control; it takes no props from the document. */
export type HostComponent = {
  description: string;
  slots?: Record<string, SlotRule>;
  render: (context: {
    slots: Record<string, ReactNode[]>;
    id: string;
  }) => ReactNode;
};

/** What the host admits for one rendering. Nothing outside it reaches the screen. */
export type Admission = {
  surface: Surface;
  /** The interface and content language; patterns and fallbacks use it. */
  locale: string;
  /** Data the host resolved for this document under the current user's rights; bindings read only this. */
  data?: unknown;
  /** Action names the host handles; an action outside this list is refused. */
  actions?: readonly string[];
  /** https origins (`https://shop.example`) that links and images may point to; relative URLs are always allowed. */
  origins?: readonly string[];
  /** Host-owned components by name; they may not shadow catalog entries. */
  host?: Readonly<Record<string, HostComponent>>;
  /** Further catalog ids the host accepts, such as its own workspace catalog. */
  catalogs?: readonly string[];
  /** The catalog id an OpenUI Lang document was prompted with. */
  openUiCatalog?: string;
};

export type ProblemCode =
  | "format"
  | "catalog"
  | "limits"
  | "unknown-component"
  | "not-admitted"
  | "invalid-props"
  | "unsafe-url"
  | "unsafe-html"
  | "data-not-admitted"
  | "slot"
  | "placement"
  | "action";

export type Problem = { node: string; code: ProblemCode; message: string };

export type BoundAction = {
  name: string;
  context?: Record<string, string | number | boolean>;
};

export type ValidNode =
  | {
      kind: "entry";
      id: string;
      entry: CatalogEntry;
      props: Record<string, unknown>;
      action?: BoundAction;
      slots: Record<string, ValidNode[]>;
    }
  | {
      kind: "host";
      id: string;
      name: string;
      host: HostComponent;
      slots: Record<string, ValidNode[]>;
    }
  | { kind: "refused"; id: string; code: ProblemCode };

export type Validation =
  | { ok: true; format: Format; root: ValidNode; problems: Problem[] }
  | { ok: false; problems: Problem[] };

const unsafeKey =
  /^(on[A-Z].*|dangerouslySetInnerHTML|innerHTML|html|style|className|class|ref|key)$/;
const isBinding = (value: unknown): value is { path: string } =>
  typeof value === "object" &&
  value !== null &&
  !Array.isArray(value) &&
  Object.keys(value).length === 1 &&
  typeof (value as { path?: unknown }).path === "string";

/** Reads a JSON Pointer (`/products/0/name`) from admitted data; `undefined` when absent. */
export function pointer(data: unknown, path: string): unknown {
  if (path === "") return data;
  if (!path.startsWith("/")) return undefined;
  let current = data;
  for (const raw of path.slice(1).split("/")) {
    const part = raw.replaceAll("~1", "/").replaceAll("~0", "~");
    if (part === "__proto__" || part === "constructor" || part === "prototype")
      return undefined;
    if (Array.isArray(current) && /^\d+$/.test(part))
      current = current[Number(part)];
    else if (
      typeof current === "object" &&
      current !== null &&
      Object.hasOwn(current, part)
    )
      current = (current as Record<string, unknown>)[part];
    else return undefined;
  }
  return current;
}

const urlProps = (entry: CatalogEntry) =>
  Object.entries(entry.props.shape)
    .filter(
      ([, field]) =>
        field === safeUrl ||
        (field as z.ZodType & { unwrap?: () => unknown }).unwrap?.() ===
          safeUrl,
    )
    .map(([key]) => key);

function classify(issue: z.core.$ZodIssue, urls: string[]): ProblemCode {
  if (
    issue.code === "unrecognized_keys" &&
    issue.keys.some((key) => unsafeKey.test(key))
  )
    return "unsafe-html";
  if (issue.message.includes("markup")) return "unsafe-html";
  if (urls.includes(String(issue.path[0]))) return "unsafe-url";
  return "invalid-props";
}

const accepts = (rule: SlotRule, child: string, kind: string | undefined) =>
  rule.accepts === "any" ||
  (rule.accepts === "inline"
    ? ["content", "atom", "molecule"].includes(kind ?? "")
    : rule.accepts.includes(child));

/** Validates `input` for `admission`. Refused nodes keep their place so the renderer can show a fallback there. */
export function validateDocument(
  input: unknown,
  admission: Admission,
): Validation {
  const host = admission.host ?? {};
  for (const name of Object.keys(host))
    if (entryByName(name))
      throw Error(`Host component ${name} shadows a catalog entry`);
  const read = readDocument(input, {
    openUiCatalog: admission.openUiCatalog,
    host,
  });
  if (!read.ok)
    return {
      ok: false,
      problems: [{ node: "root", code: read.code, message: read.message }],
    };
  const catalogs = [catalogId, ...(admission.catalogs ?? [])];
  if (!catalogs.includes(read.catalogId))
    return {
      ok: false,
      problems: [
        {
          node: "root",
          code: "catalog",
          message: `catalog ${read.catalogId} is not accepted here`,
        },
      ],
    };
  const problems: Problem[] = [];
  const actions = new Set(admission.actions ?? []);
  const origins = new Set(admission.origins ?? []);
  const refuse = (
    node: RawNode,
    code: ProblemCode,
    message: string,
  ): ValidNode => {
    problems.push({ node: node.id, code, message });
    return { kind: "refused", id: node.id, code };
  };

  const slotsOf = (
    node: RawNode,
    rules: Record<string, SlotRule>,
    parent: string,
  ): Record<string, ValidNode[]> | ProblemCode => {
    for (const slot of Object.keys(node.slots))
      if (!rules[slot]) {
        problems.push({
          node: node.id,
          code: "slot",
          message: `${parent} has no slot ${slot}`,
        });
        return "slot";
      }
    const out: Record<string, ValidNode[]> = {};
    for (const [slot, rule] of Object.entries(rules)) {
      const children = node.slots[slot] ?? [];
      if (
        children.length < (rule.min ?? 0) ||
        children.length > (rule.max ?? Infinity)
      ) {
        problems.push({
          node: node.id,
          code: "slot",
          message: `${parent}.${slot} takes ${rule.min ?? 0} to ${rule.max ?? "any number of"} children`,
        });
        return "slot";
      }
      out[slot] = children.map((child) => {
        const name = typeof child.component === "string" ? child.component : "";
        const kind = entryByName(name)?.kind;
        if (
          (Object.hasOwn(host, name) || entryByName(name)) &&
          !accepts(rule, name, kind)
        )
          return refuse(
            child,
            "slot",
            `${parent}.${slot} does not accept ${name}`,
          );
        return visit(child, parent);
      });
    }
    return out;
  };

  const visit = (node: RawNode, parent: string | null): ValidNode => {
    const name = node.component;
    if (typeof name !== "string")
      return refuse(node, "unknown-component", "the component has no name");
    if (Object.hasOwn(host, name)) {
      const component = host[name];
      if (Object.keys(node.props).length || node.action !== undefined)
        return refuse(
          node,
          "invalid-props",
          `${name} is a host component and takes no props`,
        );
      const slots = slotsOf(node, component.slots ?? {}, name);
      if (typeof slots === "string")
        return { kind: "refused", id: node.id, code: slots };
      return { kind: "host", id: node.id, name, host: component, slots };
    }
    const entry = entryByName(name);
    if (!entry)
      return refuse(
        node,
        "unknown-component",
        `${name} is not in catalog ${catalogId}`,
      );
    if (!entry.surfaces.includes(admission.surface))
      return refuse(
        node,
        "not-admitted",
        entry.codeOnly ??
          `${name} is not admitted on the ${admission.surface} surface`,
      );
    if (entry.within && (!parent || !entry.within.includes(parent)))
      return refuse(
        node,
        "placement",
        `${name} belongs inside ${entry.within.join(" or ")}`,
      );

    const props: Record<string, unknown> = {};
    for (const [key, value] of Object.entries(node.props)) {
      if (isBinding(value)) {
        const resolved = pointer(admission.data, value.path);
        if (resolved === undefined)
          return refuse(
            node,
            "data-not-admitted",
            `${name}.${key} binds ${value.path}, which the host did not admit`,
          );
        props[key] = resolved;
      } else if (entry.hostData?.includes(key)) {
        return refuse(
          node,
          "data-not-admitted",
          `${name}.${key} must be bound to host data, not stated by the document`,
        );
      } else props[key] = value;
    }
    const parsed = entry.props.safeParse(props);
    const urls = urlProps(entry);
    if (!parsed.success) {
      const issue = parsed.error.issues[0];
      return refuse(
        node,
        classify(issue, urls),
        `${name}.${issue.path.join(".") || "props"}: ${issue.message}`,
      );
    }
    for (const key of urls) {
      const value = (parsed.data as Record<string, unknown>)[key];
      const bound = isBinding(node.props[key]);
      if (
        typeof value === "string" &&
        value.startsWith("https://") &&
        !bound &&
        !origins.has(new URL(value).origin)
      )
        return refuse(
          node,
          "unsafe-url",
          `${name}.${key} points to ${new URL(value).origin}, which the host did not admit`,
        );
    }

    let action: BoundAction | undefined;
    if (node.action !== undefined) {
      if (!entry.events || !Object.keys(entry.events).length)
        return refuse(node, "action", `${name} reports no events`);
      const parsedAction = actionField.safeParse(node.action);
      if (!parsedAction.success)
        return refuse(node, "action", `${name}.action is malformed`);
      action =
        typeof parsedAction.data === "string"
          ? { name: parsedAction.data }
          : parsedAction.data;
      if (!actions.has(action.name))
        return refuse(
          node,
          "action",
          `action ${action.name} is not admitted here`,
        );
    }

    const slots = slotsOf(node, entry.slots ?? {}, name);
    if (typeof slots === "string")
      return { kind: "refused", id: node.id, code: slots };
    return {
      kind: "entry",
      id: node.id,
      entry,
      props: parsed.data as Record<string, unknown>,
      action,
      slots,
    };
  };

  return {
    ok: true,
    format: read.format,
    root: visit(read.root, null),
    problems,
  };
}
