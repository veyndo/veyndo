/** Catalog/language controls share the channel schema and content-language fallback. */
import { Button, Field, Select } from "../../../../../frontend/src/kit/ui";
import type { Channel } from "./channel-model";
import type { Category } from "../../../../catalog/ui/admin/catalog/catalog-model";
import type { RequestFn } from "../../../../../frontend/src/admin/shell/studio-types";
import { useChannelText } from "./channel-i18n";
import { useLocale } from "../../../../../frontend/src/shared/i18n/i18n";
import { contentText } from "../../../../../frontend/src/shared/i18n/content-language";
import ChannelProducts from "./ChannelProducts";
export default function ChannelCatalog({
  draft,
  languages,
  categories,
  mainLocale,
  selected,
  setSelected,
  patch,
  request,
  languageError,
  productError,
}: {
  draft: Channel;
  languages: string[];
  categories: Category[];
  mainLocale: string;
  selected: boolean;
  setSelected: (v: boolean) => void;
  patch: (v: Partial<Channel["data"]>) => void;
  request: RequestFn;
  languageError?: string;
  productError?: string;
}) {
  const t = useChannelText(),
    { locale } = useLocale();
  return (
    <>
      <h3>{t("languages")}</h3>
      <div
        className={`workbench-row ${languageError ? "field-invalid" : ""}`}
        aria-invalid={!!languageError}
      >
        {languages.map((language) => (
          <Button
            tone={
              draft.data.locales.includes(language) ? "primary" : "secondary"
            }
            type="button"
            key={language}
            disabled={draft.id === "default"}
            aria-pressed={draft.data.locales.includes(language)}
            onClick={() =>
              patch({
                locales: draft.data.locales.includes(language)
                  ? draft.data.locales.filter((l) => l !== language)
                  : [...draft.data.locales, language],
              })
            }
          >
            {new Intl.DisplayNames([locale], {
              type: "language",
            }).of(language) ?? language}
          </Button>
        ))}
      </div>
      {languageError && <small role="alert">{languageError}</small>}
      <Field label={t("root")}>
        <Select
          value={draft.data.navigationCategoryId ?? ""}
          onChange={(e) =>
            patch({ navigationCategoryId: e.target.value || null })
          }
        >
          <option value="">{t("all")}</option>
          {categories.map((c) => (
            <option key={c.id} value={c.id}>
              {contentText(
                Object.fromEntries(
                  Object.entries(c.data.translations).map(([l, v]) => [
                    l,
                    v.name,
                  ]),
                ),
                locale,
                mainLocale,
              ) || c.id}
            </option>
          ))}
        </Select>
      </Field>
      <div className="workbench-row">
        {[false, true].map((value) => (
          <Button
            tone={selected === value ? "primary" : "secondary"}
            type="button"
            key={String(value)}
            onClick={() => {
              setSelected(value);
              if (!value) patch({ productIds: [] });
            }}
          >
            {t(value ? "selected" : "all")}
          </Button>
        ))}
      </div>
      {productError && (
        <small role="alert" className="field-invalid">
          {productError}
        </small>
      )}
      {selected && (
        <ChannelProducts
          request={request}
          value={draft.data.productIds}
          onChange={(productIds) => patch({ productIds })}
        />
      )}
    </>
  );
}
