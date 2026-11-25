/** Versioned setting declarations share the native Manifest; no credential value enters a draft. */
import { Button, Field, Select, TextInput } from "../../kit/ui";
import type { Manifest } from "../../shared/apps/native/types";
import LocalizedField from "../../shared/i18n/LocalizedField";
import { useAppSettingText } from "../../shared/i18n/app-settings-i18n";
import { useAppStudioText } from "../../shared/i18n/app-studio-i18n";
import { nextId, textMap } from "./app-model";
export default function AppSettingsEditor({
  manifest,
  onChange,
}: {
  manifest: Manifest;
  onChange: (m: Manifest) => void;
}) {
  const t = useAppSettingText(),
    { a } = useAppStudioText(),
    fields = manifest.settings ?? [];
  return (
    <section className="app-settings-editor">
      <h2>{t("settings")}</h2>
      <p>{t("hint")}</p>
      <p>{t("runtime")}</p>
      {fields.map((f, i) => (
        <article className="app-model-card" key={i}>
          <div className="app-key-fields">
            <Field label={t("key")}>
              <TextInput
                value={f.key}
                maxLength={32}
                onChange={(e) =>
                  onChange({
                    ...manifest,
                    settings: fields.map((v, n) =>
                      n === i ? { ...v, key: e.target.value } : v,
                    ),
                  })
                }
              />
            </Field>
            <LocalizedField
              label={t("label")}
              value={f.label}
              onChange={(label) =>
                onChange({
                  ...manifest,
                  settings: fields.map((v, n) =>
                    n === i ? { ...v, label: textMap(label) } : v,
                  ),
                })
              }
              maxLength={100}
            />
            <Field label={a("type")}>
              <Select
                value={f.kind}
                onChange={(e) =>
                  onChange({
                    ...manifest,
                    settings: fields.map((v, n) =>
                      n === i
                        ? { ...v, kind: e.target.value as typeof f.kind }
                        : v,
                    ),
                  })
                }
              >
                {(["string", "secret", "boolean", "integer"] as const).map(
                  (k) => (
                    <option key={k} value={k}>
                      {t(k)}
                    </option>
                  ),
                )}
              </Select>
            </Field>
            <label className="app-check">
              <input
                type="checkbox"
                checked={f.required}
                onChange={(e) =>
                  onChange({
                    ...manifest,
                    settings: fields.map((v, n) =>
                      n === i ? { ...v, required: e.target.checked } : v,
                    ),
                  })
                }
              />
              {t("required")}
            </label>
            <Button
              type="button"
              onClick={() =>
                onChange({
                  ...manifest,
                  settings: fields.filter((_, n) => n !== i),
                })
              }
            >
              {a("delete")}
            </Button>
          </div>
        </article>
      ))}
      <Button
        type="button"
        disabled={fields.length >= 16}
        onClick={() =>
          onChange({
            ...manifest,
            settings: [
              ...fields,
              {
                key: nextId(
                  "setting",
                  fields.map((f) => f.key),
                ),
                label: {},
                kind: "string",
                required: false,
              },
            ],
          })
        }
      >
        {t("add")}
      </Button>
    </section>
  );
}
