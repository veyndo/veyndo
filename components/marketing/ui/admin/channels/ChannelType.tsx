/** Channel kind and POS presentation editor use the existing channel/content-language contract. */
import { useRead } from "../../../../../frontend/src/admin/shell/useRead";
import { Field, Select } from "../../../../../frontend/src/kit/ui";
import CountryPicker from "../../../../../frontend/src/shared/geography/CountryPicker";
import type { Country } from "../../../../../frontend/src/shared/geography/geography-types";
import type { RequestFn } from "../../../../../frontend/src/admin/shell/studio-types";
import type { Channel } from "./channel-model";
import { useChannelText } from "./channel-i18n";
import Icon from "../../../../../frontend/src/shared/ui/Icon";
import LocalizedField from "../../../../../frontend/src/shared/i18n/LocalizedField";
export default function ChannelType({
  request,
  draft,
  patch,
  countries,
  countryError,
}: {
  request: RequestFn;
  draft: Channel;
  patch: (v: Partial<Channel["data"]>) => void;
  countries: string[];
  countryError?: string;
}) {
  const t = useChannelText();
  const catalogue = useRead(request, "/store-api/countries").data?.countries;
  const world: Country[] = Array.isArray(catalogue) ? catalogue : [];
  const config = draft.data.pos ?? {
    country: countries[0] ?? "",
    columns: 4,
    compact: false,
    receiptMessage: {},
  };
  return (
    <>
      <div className="channel-kind-grid" role="group" aria-label={t("type")}>
        {(["storefront", "headless", "pos", "marketplace"] as const).map(
          (kind) => (
            <button
              type="button"
              key={kind}
              className="channel-kind"
              disabled={
                (draft.id === "default" && kind !== "storefront") ||
                (!!draft.revision &&
                  (["pos", "marketplace"].includes(draft.data.kind) ||
                    ["pos", "marketplace"].includes(kind)))
              }
              aria-pressed={draft.data.kind === kind}
              onClick={() =>
                patch({ kind, pos: kind === "pos" ? config : undefined })
              }
            >
              <span className="channel-kind-icon">
                <Icon
                  name={
                    kind === "marketplace"
                      ? "layers"
                      : kind === "pos"
                        ? "register"
                        : kind === "headless"
                          ? "code"
                          : "globe"
                  }
                />
              </span>
              <span className="channel-kind-copy">
                <strong>{t(kind)}</strong>
                <p>
                  {t(
                    kind === "marketplace"
                      ? "marketplaceHint"
                      : kind === "pos"
                        ? "posHint"
                        : kind === "storefront"
                          ? "storefrontHint"
                          : "headlessHint",
                  )}
                </p>
              </span>
              {draft.data.kind === kind && (
                <span className="channel-kind-selected">
                  <Icon name="check" size={18} />
                </span>
              )}
            </button>
          ),
        )}
      </div>
      {draft.data.kind === "pos" && (
        <section className="commerce-fields">
          <div className={countryError ? "field-invalid" : ""}>
            <CountryPicker
              countries={world.filter((c) => countries.includes(c.code))}
              label={t("posCountry")}
              value={[config.country]}
              single
              onChange={(value) =>
                patch({ pos: { ...config, country: value[0] ?? "" } })
              }
            />
            {countryError && <small role="alert">{countryError}</small>}
          </div>
          <Field label={t("posColumns")}>
            <Select
              value={config.columns}
              onChange={(e) =>
                patch({ pos: { ...config, columns: Number(e.target.value) } })
              }
            >
              {[2, 3, 4, 5, 6].map((n) => (
                <option key={n} value={n}>
                  {n}
                </option>
              ))}
            </Select>
          </Field>
          <label>
            <input
              type="checkbox"
              checked={config.compact}
              onChange={(e) =>
                patch({ pos: { ...config, compact: e.target.checked } })
              }
            />
            {t("posCompact")}
          </label>
          <LocalizedField
            label={t("posMessage")}
            value={config.receiptMessage}
            maxLength={500}
            onChange={(value) =>
              patch({
                pos: {
                  ...config,
                  receiptMessage: value as Record<string, string>,
                },
              })
            }
          />
          <p>{t("posRegisterHint")}</p>
        </section>
      )}
    </>
  );
}
