/** One saved predicate on current public evidence; reuses category CAS and the editor's selected content language. */
import { Card, Field, Select, TextInput } from "../../kit/ui";
import type { Category } from "./catalog-model";
import LocalizedField from "../../shared/i18n/LocalizedField";
import { useContentLanguage } from "../../shared/i18n/ContentLanguage";
import { useGraphNavigationText } from "../../shared/i18n/graph-navigation-i18n";
export default function CategoryFactQuery({
  value,
  onChange,
}: {
  value: Category["data"]["graphQuery"];
  onChange: (query: Category["data"]["graphQuery"]) => void;
}) {
  const g = useGraphNavigationText();
  const { mainLocale } = useContentLanguage();
  return (
    <Card>
      <label className="checkbox-label">
        <input
          type="checkbox"
          checked={!!value}
          onChange={(e) =>
            onChange(
              e.target.checked
                ? {
                    nodeType: "intent",
                    minimumConfidence: 0.5,
                    text: { [mainLocale]: "" },
                  }
                : null,
            )
          }
        />
        {g("enabled")}
      </label>
      <p>{g("hint")}</p>
      {value && (
        <div className="catalog-form-grid">
          <Field label={g("kind")}>
            <Select
              value={value.nodeType}
              onChange={(e) =>
                onChange({
                  ...value,
                  nodeType: e.target.value as NonNullable<
                    typeof value
                  >["nodeType"],
                })
              }
            >
              {(
                [
                  "intent",
                  "problem",
                  "occasion",
                  "audience",
                  "material",
                  "property",
                ] as const
              ).map((kind) => (
                <option value={kind} key={kind}>
                  {g(kind)}
                </option>
              ))}
            </Select>
          </Field>
          <Field label={g("confidence")}>
            <TextInput
              type="number"
              min="0"
              max="1"
              step="0.05"
              value={value.minimumConfidence}
              onChange={(e) =>
                onChange({
                  ...value,
                  minimumConfidence: Number(e.target.value),
                })
              }
            />
          </Field>
          <LocalizedField
            label={g("phrase")}
            value={value.text}
            onChange={(text) => onChange({ ...value, text })}
            maxLength={120}
            required
          />
        </div>
      )}
    </Card>
  );
}
