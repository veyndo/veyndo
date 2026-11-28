/** The living component gallery: every catalog entry's contract and examples, rendered through CatalogView in both themes. */
import { useMemo, useState } from "react";
import { Badge, Field, Select, Stack, TextInput } from "../ui";
import { catalogDocument, catalogId, catalogVersion, entries } from "./catalog";
import { catalogText, galleryLocales, sampleText } from "./catalog-i18n";
import { CatalogView } from "./CatalogView";
import type { CatalogEntry, Example } from "./define";

type Schema = Record<string, unknown>;

/** A one-line type label of a JSON Schema fragment for the props table. */
export function typeLabel(schema: Schema): string {
  if (Array.isArray(schema.enum))
    return schema.enum.map((value) => JSON.stringify(value)).join(" | ");
  const union = (schema.anyOf ?? schema.oneOf) as Schema[] | undefined;
  if (union) return union.map(typeLabel).join(" | ");
  if (schema.type === "array")
    return `${typeLabel((schema.items ?? {}) as Schema)}[]`;
  if (schema.format === "safe-url") return "url";
  return typeof schema.type === "string"
    ? schema.type === "integer"
      ? "number"
      : schema.type
    : "unknown";
}

/** The document an example renders, as a host would receive it. */
export const exampleDocument = (example: Example, locale: string) => ({
  format: "veyndo-ui/1",
  catalog: catalogId,
  root: example.node(sampleText(locale)),
});

/** The admission a gallery example renders with: its entry's first surface, sample data and actions. */
export const exampleAdmission = (
  entry: CatalogEntry,
  example: Example,
  locale: string,
) => ({
  surface: entry.surfaces[0] ?? "studio",
  locale,
  data: example.data?.(sampleText(locale)),
  actions: example.actions ?? [],
});

function ExampleFrames({
  entry,
  example,
  locale,
}: {
  entry: CatalogEntry;
  example: Example;
  locale: string;
}) {
  const document = useMemo(
    () => exampleDocument(example, locale),
    [example, locale],
  );
  const admission = useMemo(
    () => exampleAdmission(entry, example, locale),
    [entry, example, locale],
  );
  const t = catalogText(locale);
  return (
    <figure className="vy-gallery-example">
      <figcaption>
        <code>{example.id}</code> <Badge>{example.state}</Badge>
      </figcaption>
      <div className="vy-gallery-frames">
        {(["light", "dark"] as const).map((theme) => (
          <div
            key={theme}
            className="vy-gallery-frame"
            data-theme={theme}
            data-gallery-example={`${entry.name}/${example.id}/${theme}`}
            aria-label={`${entry.name} ${example.state} ${t(theme)}`}
            role="group"
          >
            <CatalogView
              document={document}
              admission={admission}
              onAction={() => undefined}
            />
          </div>
        ))}
      </div>
    </figure>
  );
}

function EntrySection({
  entry,
  contract,
  locale,
  uiLocale,
}: {
  entry: CatalogEntry;
  contract: ReturnType<typeof catalogDocument>["entries"][number];
  locale: string;
  uiLocale: string;
}) {
  const t = catalogText(uiLocale);
  const properties = (contract.props.properties ?? {}) as Record<
    string,
    Schema
  >;
  const required = new Set((contract.props.required ?? []) as string[]);
  return (
    <section
      className="vy-gallery-entry"
      id={`component-${entry.name}`}
      aria-labelledby={`component-${entry.name}-title`}
    >
      <header>
        <h2 id={`component-${entry.name}-title`}>{entry.name}</h2>
        <Badge tone={entry.kind === "pattern" ? "accent" : "neutral"}>
          {entry.kind}
        </Badge>
        {entry.surfaces.length ? (
          entry.surfaces.map((surface) => (
            <Badge key={surface}>{surface}</Badge>
          ))
        ) : (
          <Badge tone="warning">{t("codeOnly")}</Badge>
        )}
      </header>
      <p>{entry.description}</p>
      {entry.codeOnly && <p className="vy-gallery-muted">{entry.codeOnly}</p>}
      <dl className="vy-gallery-contract">
        <dt>{t("props")}</dt>
        <dd>
          <ul>
            {Object.entries(properties).map(([prop, schema]) => (
              <li key={prop}>
                <code>
                  {prop}
                  {required.has(prop) ? "" : "?"}
                </code>
                : <code>{typeLabel(schema)}</code>
                {schema.default !== undefined && (
                  <>
                    {" "}
                    = <code>{JSON.stringify(schema.default)}</code>
                  </>
                )}
                {contract.hostData?.includes(prop) && (
                  <>
                    {" "}
                    <Badge tone="accent">{t("hostData")}</Badge>
                  </>
                )}
              </li>
            ))}
          </ul>
        </dd>
        {Object.keys(contract.slots).length > 0 && (
          <>
            <dt>{t("slots")}</dt>
            <dd>
              <ul>
                {Object.entries(contract.slots).map(([slot, rule]) => (
                  <li key={slot}>
                    <code>{slot}</code>: {rule.description}{" "}
                    <code>
                      {Array.isArray(rule.accepts)
                        ? rule.accepts.join(", ")
                        : rule.accepts}
                    </code>
                  </li>
                ))}
              </ul>
            </dd>
          </>
        )}
        {Object.keys(contract.events).length > 0 && (
          <>
            <dt>{t("events")}</dt>
            <dd>
              <ul>
                {Object.entries(contract.events).map(([event, rule]) => (
                  <li key={event}>
                    <code>{event}</code>: {rule.description}
                  </li>
                ))}
              </ul>
            </dd>
          </>
        )}
        <dt>{t("accessibility")}</dt>
        <dd>
          {entry.a11y.role && <code>{entry.a11y.role}</code>} {entry.a11y.notes}
          {entry.a11y.keyboard && (
            <>
              {" "}
              <code>{entry.a11y.keyboard.join(" · ")}</code>
            </>
          )}
        </dd>
      </dl>
      {entry.examples.length > 0
        ? entry.examples.map((example) => (
            <ExampleFrames
              key={example.id}
              entry={entry}
              example={example}
              locale={locale}
            />
          ))
        : entry.surfaces.length > 0 && (
            <p className="vy-gallery-muted">{t("noExamples")}</p>
          )}
    </section>
  );
}

/** The gallery. `locale` is the interface language; samples can be switched to any shipped language. */
export function Gallery({ locale }: { locale: string }) {
  const t = catalogText(locale);
  const [sample, setSample] = useState<string>(
    galleryLocales.find(
      (candidate) => candidate.slice(0, 2) === locale.slice(0, 2),
    ) ?? "en-GB",
  );
  const [query, setQuery] = useState("");
  const contract = useMemo(() => catalogDocument(), []);
  const shown = entries.filter((entry) =>
    entry.name.toLowerCase().includes(query.trim().toLowerCase()),
  );
  return (
    <div className="vy-gallery">
      <header className="vy-gallery-head">
        <h1>{t("galleryTitle")}</h1>
        <p>{t("galleryIntro")}</p>
        <p>
          <code>{catalogId}</code> <Badge tone="accent">{catalogVersion}</Badge>
        </p>
        <Stack direction="row" gap="4" wrap>
          <Field label={t("filter")}>
            <TextInput
              type="search"
              value={query}
              onChange={(event) => setQuery(event.target.value)}
            />
          </Field>
          <Field label={t("language")}>
            <Select
              value={sample}
              onChange={(event) => setSample(event.target.value)}
            >
              {galleryLocales.map((candidate) => (
                <option key={candidate} value={candidate}>
                  {new Intl.DisplayNames([locale], { type: "language" }).of(
                    candidate.slice(0, 2),
                  ) ?? candidate}
                </option>
              ))}
            </Select>
          </Field>
        </Stack>
      </header>
      {shown.map((entry) => (
        <EntrySection
          key={entry.name}
          entry={entry}
          contract={contract.entries.find(
            (candidate) => candidate.name === entry.name,
          )!}
          locale={sample}
          uiLocale={locale}
        />
      ))}
    </div>
  );
}
