/** The versioned component catalog: its entries, the catalog document agents read and the generated TypeScript prop types. */
import { z } from "zod";
import {
  openUiArguments,
  surfaces,
  type CatalogEntry,
  type Sample,
} from "./define";
import { uiEntries } from "./entries";
import { patternEntries } from "./patterns";

/** Documents name this id (A2UI `catalogId`); the number is the major version. */
export const catalogId = "veyndo:components/1";
/** Raised for every additive change; the major part changes only with a new `catalogId`. */
export const catalogVersion = "1.0.0";
/** Bounds every document must stay within, whatever its format. */
export const limits = {
  bytes: 262144,
  nodes: 400,
  depth: 16,
} as const;

export const entries: readonly CatalogEntry[] = [
  ...uiEntries,
  ...patternEntries,
];
const byName = new Map(entries.map((entry) => [entry.name, entry]));
if (byName.size !== entries.length) throw Error("Duplicate catalog entry");

/** The entry called `name`, if the catalog has it. */
export const entryByName = (name: string) => byName.get(name);

type JsonSchema = Record<string, unknown>;
const schemaOf = (schema: z.ZodType): JsonSchema => {
  const { $schema: _, ...rest } = z.toJSONSchema(schema, {
    target: "draft-2020-12",
    io: "input",
    unrepresentable: "any",
  }) as JsonSchema;
  return rest;
};

/** The catalog document: what a renderer accepts, in a form any language can read. Examples use English sample text. */
export function catalogDocument(sample: Sample = (key) => key) {
  return {
    id: catalogId,
    version: catalogVersion,
    formats: ["veyndo-ui/1", "a2ui/1.0", "openui-lang"],
    surfaces,
    limits,
    entries: entries.map((entry) => ({
      name: entry.name,
      kind: entry.kind,
      description: entry.description,
      source: entry.source,
      since: entry.since,
      surfaces: entry.surfaces,
      ...(entry.codeOnly ? { codeOnly: entry.codeOnly } : {}),
      ...(entry.within ? { within: entry.within } : {}),
      props: schemaOf(entry.props),
      ...(entry.hostData?.length ? { hostData: entry.hostData } : {}),
      slots: Object.fromEntries(
        Object.entries(entry.slots ?? {}).map(([slot, rule]) => [
          slot,
          {
            description: rule.description,
            accepts: rule.accepts,
            ...(rule.min !== undefined ? { min: rule.min } : {}),
            ...(rule.max !== undefined ? { max: rule.max } : {}),
          },
        ]),
      ),
      events: Object.fromEntries(
        Object.entries(entry.events ?? {}).map(([event, rule]) => [
          event,
          {
            description: rule.description,
            ...(rule.payload ? { payload: schemaOf(rule.payload) } : {}),
          },
        ]),
      ),
      a11y: entry.a11y,
      ...(entry.surfaces.length
        ? { openUi: { arguments: openUiArguments(entry) } }
        : {}),
      examples: entry.examples.map((example) => ({
        id: example.id,
        state: example.state,
        root: example.node(sample),
      })),
    })),
  };
}

const identifierPattern = /^[A-Za-z_$][A-Za-z0-9_$]*$/;
const key = (name: string) =>
  identifierPattern.test(name) ? name : JSON.stringify(name);

/** TypeScript for a JSON Schema fragment as `z.toJSONSchema` writes it. */
function typeOf(schema: JsonSchema, indent: string): string {
  if (Array.isArray(schema.enum))
    return schema.enum.map((value) => JSON.stringify(value)).join(" | ");
  if ("const" in schema) return JSON.stringify(schema.const);
  const union = (schema.anyOf ?? schema.oneOf) as JsonSchema[] | undefined;
  if (union) return union.map((part) => typeOf(part, indent)).join(" | ");
  switch (schema.type) {
    case "string":
      return "string";
    case "number":
    case "integer":
      return "number";
    case "boolean":
      return "boolean";
    case "array": {
      const item = typeOf((schema.items ?? {}) as JsonSchema, indent);
      return /[|&]/.test(item) ? `(${item})[]` : `${item}[]`;
    }
    case "object": {
      const properties = (schema.properties ?? {}) as Record<
        string,
        JsonSchema
      >;
      const required = new Set((schema.required ?? []) as string[]);
      const names = Object.keys(properties);
      if (!names.length) {
        const extra = schema.additionalProperties;
        return extra && typeof extra === "object"
          ? `Record<string, ${typeOf(extra as JsonSchema, indent)}>`
          : "Record<string, never>";
      }
      const inner = indent + "  ";
      return `{\n${names
        .map((name) => {
          const property = properties[name];
          const doc = property.description
            ? `${inner}/** ${String(property.description)} */\n`
            : "";
          return `${doc}${inner}${key(name)}${required.has(name) ? "" : "?"}: ${typeOf(property, inner)};`;
        })
        .join("\n")}\n${indent}}`;
    }
    default:
      return "unknown";
  }
}

/** The generated TypeScript module: document prop types per entry and the typed document node. */
export function propTypesSource() {
  const document = catalogDocument();
  const lines = [
    `/** Generated from kit/catalog by \`WRITE_COMPONENT_CATALOG=1 npm test -- component-catalog\`; do not edit. Document prop types of catalog ${catalogId} (${catalogVersion}). */`,
    `export const catalogId = ${JSON.stringify(catalogId)};`,
    `export const catalogVersion = ${JSON.stringify(catalogVersion)};`,
    "",
  ];
  for (const entry of document.entries) {
    lines.push(`/** ${entry.description} */`);
    lines.push(
      `export type ${entry.name}DocumentProps = ${typeOf(entry.props, "")};`,
    );
    lines.push("");
  }
  lines.push(
    "/** An action a document binds to an interactive component; the host admits its name. */",
    "export type DocumentAction = string | { name: string; context?: Record<string, string | number | boolean> };",
    "/** A value bound to host-admitted data by JSON Pointer. */",
    "export type DocumentBinding = { path: string };",
    "type Bindable<P> = { [K in keyof P]: P[K] | DocumentBinding };",
    "/** One node of a `veyndo-ui/1` document. */",
    "export type DocumentNode =",
    ...document.entries
      .filter((entry) => entry.surfaces.length)
      .map((entry) => {
        const slots = Object.keys(entry.slots).filter(
          (slot) => slot !== "children",
        );
        const parts = [
          `component: ${JSON.stringify(entry.name)}`,
          `props?: Bindable<${entry.name}DocumentProps>`,
          ...(Object.keys(entry.events).length
            ? ["action?: DocumentAction"]
            : []),
          ...("children" in entry.slots ? ["children?: DocumentNode[]"] : []),
          ...(slots.length
            ? [
                `slots?: { ${slots.map((slot) => `${slot}?: DocumentNode[]`).join("; ")} }`,
              ]
            : []),
        ];
        return `  | { ${parts.join("; ")} }`;
      }),
    ";",
    "/** A complete `veyndo-ui/1` document. */",
    `export type UiDocument = { format: "veyndo-ui/1"; catalog: typeof catalogId; root: DocumentNode };`,
    "",
  );
  return lines.join("\n");
}
