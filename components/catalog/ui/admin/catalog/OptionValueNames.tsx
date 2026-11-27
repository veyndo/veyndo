/** Names of a family's option values in the editor's one content language, stored on the family root (`extra.optionValues`); a missing name inherits the main language, never a copied text. */
import LocalizedField from "../../../../../frontend/src/shared/i18n/LocalizedField";
import { useContentLanguage } from "../../../../../frontend/src/shared/i18n/ContentLanguage";
import {
  contentText,
  type LocalizedText,
} from "../../../../../frontend/src/shared/i18n/content-language";
import type { ProductDraft } from "./catalog-model";
import { useVariantText } from "./variant-i18n";

type Names = Record<string, Record<string, LocalizedText>>;

/** Drops blank and inherited entries so only names the merchant wrote are saved. */
export function withName(
  names: Names,
  group: string,
  value: string,
  text: LocalizedText,
): Names {
  const kept = Object.fromEntries(
    Object.entries(text).filter(
      ([, v]) => typeof v === "string" && v.trim() !== "",
    ),
  );
  const values = { ...names[group], [value]: kept };
  if (!Object.keys(kept).length) delete values[value];
  const next = { ...names, [group]: values };
  if (!Object.keys(values).length) delete next[group];
  return next;
}

/** Every option value of the family root and its loaded variants, grouped in first-seen order. */
export function familyValues(options: Record<string, string>[]) {
  const groups: Record<string, string[]> = {};
  for (const set of options)
    for (const [group, value] of Object.entries(set ?? {}))
      if (!(groups[group] ??= []).includes(value)) groups[group].push(value);
  return Object.entries(groups);
}

export default function OptionValueNames({
  draft,
  variants,
  onChange,
  canWrite = true,
}: {
  draft: ProductDraft;
  variants: { options: Record<string, string> }[];
  onChange: (draft: ProductDraft) => void;
  canWrite?: boolean;
}) {
  const v = useVariantText(),
    { language, mainLocale } = useContentLanguage();
  const groups = familyValues([
    draft.catalog.options,
    ...variants.map((p) => p.options),
  ]);
  if (!groups.length) return null;
  const names: Names = draft.extra.optionValues ?? {};
  return (
    <section className="variant-option-names">
      <h3>{v("optionNames")}</h3>
      <p className="variant-family-hint">{v("optionNamesHint")}</p>
      {groups.map(([group, values]) => (
        <fieldset
          key={group}
          className="catalog-permission-fields"
          disabled={!canWrite}
        >
          <legend>{group}</legend>
          {values.map((value) => {
            const text = names[group]?.[value] ?? {};
            const mainName = contentText(text, mainLocale, mainLocale);
            return (
              <LocalizedField
                key={`${group}:${value}`}
                label={value}
                value={text}
                maxLength={200}
                disabled={!canWrite}
                externalFallback={
                  language === mainLocale || !mainName ? value : undefined
                }
                externalLabel={v("usesValue")}
                onChange={(map) =>
                  onChange({
                    ...draft,
                    extra: {
                      ...draft.extra,
                      optionValues: withName(names, group, value, map),
                    },
                  })
                }
              />
            );
          })}
        </fieldset>
      ))}
    </section>
  );
}
