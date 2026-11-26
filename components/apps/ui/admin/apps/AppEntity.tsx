/** Managed entity editor renders fields from the installed app contract. */
import type { ApiPath } from "../../../../../frontend/src/kit/api";
import {
  Button,
  Field,
  Notice,
  TextInput,
} from "../../../../../frontend/src/kit/ui";
import { useEffect, useRef, useState } from "react";
import { useAppText } from "../../../../../frontend/src/shared/i18n/app-i18n";
import type { RequestFn } from "../../../../../frontend/src/admin/shell/studio-types";
import {
  useRead,
  useSetRead,
} from "../../../../../frontend/src/admin/shell/useRead";
import { useContentLanguage } from "../../../../../frontend/src/shared/i18n/ContentLanguage";
import { contentText } from "../../../../../frontend/src/shared/i18n/content-language";
import LocalizedField from "../../../../../frontend/src/shared/i18n/LocalizedField";
export type Entity = {
  name: string;
  label?: Record<string, string>;
  fields: {
    name: string;
    label?: Record<string, string>;
    translatable?: boolean;
    kind: string;
    required: boolean;
  }[];
};
export default function AppEntity({
  app,
  entity,
  request,
  canWrite,
}: {
  app: string;
  entity: Entity;
  request: RequestFn;
  canWrite: boolean;
}) {
  const { a } = useAppText();
  const { language, mainLocale } = useContentLanguage();
  const label = (v: { name: string; label?: Record<string, string> }) =>
    contentText(v.label ?? {}, language, mainLocale) || a(v.name);
  const fieldValue = (f: Entity["fields"][number], value: unknown) =>
    f.translatable && value && typeof value === "object"
      ? contentText(value as Record<string, string>, language, mainLocale)
      : value;
  const [id, setId] = useState("default");
  const [revision, setRevision] = useState(0);
  const [fields, setFields] = useState<Record<string, unknown>>({});
  const [actionError, setError] = useState("");
  const [busy, setBusy] = useState(false);
  const recordsPath: ApiPath = `/api/apps/${app}/entities/${entity.name}`;
  const recordsRead = useRead(request, recordsPath);
  const setRead = useSetRead(request);
  const records: Record<string, unknown>[] = recordsRead.data?.elements ?? [];
  const error =
    actionError || (recordsRead.error as Error | undefined)?.message || "";
  const load = async () => setRead(recordsPath, await request(recordsPath));
  // Open the first record once per entity, when its list first arrives.
  const opened = useRef("");
  useEffect(() => {
    const r = recordsRead.data?.elements?.[0];
    if (!r || opened.current === recordsPath) return;
    opened.current = recordsPath;
    setId(r.id);
    setRevision(r.revision);
    setFields(
      Object.fromEntries(entity.fields.map((f) => [f.name, r[f.name]])),
    );
  }, [recordsPath, recordsRead.data, entity.fields]);
  return (
    <section className="app-entity">
      <h3>
        {a("records")} · {label(entity)}
      </h3>
      <div className="app-records">
        {records.map((r) => (
          <button
            key={String(r.id)}
            onClick={() => {
              setId(String(r.id));
              setRevision(Number(r.revision));
              setFields(
                Object.fromEntries(
                  entity.fields.map((f) => [f.name, r[f.name]]),
                ),
              );
            }}
          >
            {String(r.id)} ·{" "}
            {entity.fields
              .map((f) => `${label(f)}: ${fieldValue(f, r[f.name]) ?? "—"}`)
              .join(" · ")}
          </button>
        ))}
      </div>
      <form
        onSubmit={async (e) => {
          e.preventDefault();
          setBusy(true);
          setError("");
          try {
            const values = { ...fields };
            for (const f of entity.fields)
              if (f.kind === "json" && typeof values[f.name] === "string")
                values[f.name] = JSON.parse(values[f.name] as string);
            const v = await request(
              `/api/apps/${app}/entities/${entity.name}`,
              { id, revision, fields: values },
            );
            setRevision(v.revision);
            await load();
          } catch (e) {
            setError((e as Error).message);
          } finally {
            setBusy(false);
          }
        }}
      >
        <Field label={a("recordId")}>
          <TextInput
            value={id}
            onChange={(e) => {
              setId(e.target.value);
              setRevision(
                (records.find((r) => r.id === e.target.value)
                  ?.revision as number) ?? 0,
              );
            }}
            required
            maxLength={100}
          />
        </Field>
        {entity.fields.map((f) =>
          f.translatable ? (
            <LocalizedField
              key={f.name}
              label={label(f)}
              value={
                typeof fields[f.name] === "object" && fields[f.name] !== null
                  ? (fields[f.name] as Record<string, string>)
                  : {}
              }
              onChange={(value) => setFields({ ...fields, [f.name]: value })}
              required={f.required}
              maxLength={2000}
              disabled={!canWrite || busy}
            />
          ) : (
            <label key={f.name}>
              {label(f)}
              {f.kind === "json" ? (
                <textarea
                  disabled={!canWrite || busy}
                  value={
                    typeof fields[f.name] === "string"
                      ? (fields[f.name] as string)
                      : JSON.stringify(fields[f.name] ?? {}, null, 2)
                  }
                  maxLength={8192}
                  required={f.required}
                  onChange={(e) =>
                    setFields({ ...fields, [f.name]: e.target.value })
                  }
                />
              ) : (
                <input
                  disabled={!canWrite || busy}
                  type={
                    f.kind === "integer"
                      ? "number"
                      : f.kind === "boolean"
                        ? "checkbox"
                        : "text"
                  }
                  value={
                    f.kind === "boolean"
                      ? undefined
                      : String(fieldValue(f, fields[f.name]) ?? "")
                  }
                  checked={
                    f.kind === "boolean" ? Boolean(fields[f.name]) : undefined
                  }
                  required={f.required && f.kind !== "boolean"}
                  maxLength={2000}
                  onChange={(e) =>
                    setFields({
                      ...fields,
                      [f.name]:
                        f.kind === "integer"
                          ? Number(e.target.value)
                          : f.kind === "boolean"
                            ? e.target.checked
                            : e.target.value,
                    })
                  }
                />
              )}
            </label>
          ),
        )}
        <Button tone="primary" type="submit" disabled={!canWrite || busy}>
          {a("save")}
        </Button>
      </form>
      {error && <Notice tone="danger">{error}</Notice>}
    </section>
  );
}
