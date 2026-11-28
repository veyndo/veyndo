/** Shared page/channel appearance controls; null values retain native inheritance. Home pages also carry the brand theme and motion. */
import { useState } from "react";
import type { RequestFn } from "../../../../frontend/src/shared/api/types";
import { Button } from "../../../../frontend/src/kit/ui";
import { motionIntensities } from "../../../../frontend/src/kit/tokens";
import { defaultTheme, explicitTheme, type Theme } from "../model";
import type { SiteProduct } from "../Renderer";
import { useDesignText } from "./design-i18n";
import { useBrandText } from "./brand-i18n";
import BrandThemeDialog from "./BrandThemeDialog";

/** What the brand dialog needs to read the shop: present only for home pages. */
export type BrandContext = {
  request: RequestFn;
  products: SiteProduct[];
  channel: string;
  language: string;
  workspace: string;
};
/** Removing a brand theme returns every field it set to inheritance. */
const unbranded: Theme = {
  brand: null,
  accent: null,
  background: null,
  foreground: null,
  font: null,
  radius: null,
  spacing: null,
};
export default function AppearanceControls({
  value,
  appearance,
  open,
  onChange,
  brand,
}: {
  value: Theme;
  appearance?: Theme;
  open: boolean;
  onChange: (value: Theme) => void;
  brand?: BrandContext;
}) {
  const t = useDesignText(),
    b = useBrandText();
  const [dialog, setDialog] = useState(false);
  const patch = (next: Theme) => onChange({ ...value, ...next });
  const branded = value.brand ?? undefined;
  const effective = {
    ...defaultTheme,
    ...explicitTheme(appearance),
    ...explicitTheme(value),
  };
  return (
    <details open={open}>
      <summary>{t("theme")}</summary>
      {brand ? (
        <section className="designer-brand">
          {branded ? (
            <>
              <p>
                <strong>{b("brandActive")}</strong>
              </p>
              <div className="brand-palette" aria-hidden="true">
                {[
                  branded.colors.accent,
                  branded.colors.text,
                  branded.colors.bg,
                ].map((c, i) => (
                  <span key={i} style={{ background: c }} />
                ))}
              </div>
              <Button onClick={() => setDialog(true)}>{b("editBrand")}</Button>
              <Button tone="ghost" onClick={() => patch(unbranded)}>
                {b("removeBrand")}
              </Button>
            </>
          ) : (
            <Button tone="primary" onClick={() => setDialog(true)}>
              {b("createBrand")}
            </Button>
          )}
          <label>
            {b("motion")}
            <select
              value={value.motion ?? ""}
              onChange={(event) =>
                patch({
                  motion: (event.target.value || null) as Theme["motion"],
                })
              }
            >
              <option value="">{b("inheritMotion")}</option>
              {motionIntensities.map((m) => (
                <option key={m} value={m}>
                  {b(m)}
                </option>
              ))}
            </select>
          </label>
          <p className="designer-hint">{b("motionHint")}</p>
        </section>
      ) : (
        <p className="designer-hint">{b("homeOnly")}</p>
      )}
      {!branded &&
        (["accent", "background", "foreground"] as const).map((k) => (
          <label className="designer-color" key={k}>
            {t(k)}
            <input
              type="color"
              value={
                value[k] ?? appearance?.[k] ?? defaultTheme[k] ?? "#ffffff"
              }
              onChange={(event) => patch({ [k]: event.target.value })}
            />
          </label>
        ))}
      {!branded && (
        <label>
          {t("font")}
          <select
            value={value.font ?? appearance?.font ?? "sans"}
            onChange={(event) =>
              patch({ font: event.target.value as Theme["font"] })
            }
          >
            <option value="sans">{t("sans")}</option>
            <option value="serif">{t("serif")}</option>
            <option value="mono">{t("mono")}</option>
          </select>
        </label>
      )}
      {(branded
        ? (["columns"] as const)
        : (["radius", "spacing", "columns"] as const)
      ).map((k) => (
        <label key={k}>
          {t(k)} · {value[k] ?? appearance?.[k] ?? defaultTheme[k]}
          <input
            type="range"
            min={k === "columns" ? 1 : k === "spacing" ? 12 : 0}
            max={k === "columns" ? 4 : k === "spacing" ? 96 : 32}
            value={value[k] ?? appearance?.[k] ?? defaultTheme[k] ?? 0}
            onChange={(event) => patch({ [k]: Number(event.target.value) })}
          />
        </label>
      ))}
      <button onClick={() => onChange({})}>{t("inherit")}</button>
      {dialog && brand && (
        <BrandThemeDialog
          {...brand}
          current={effective}
          onClose={() => setDialog(false)}
          onApply={(theme) => {
            patch(theme);
            setDialog(false);
          }}
        />
      )}
    </details>
  );
}
