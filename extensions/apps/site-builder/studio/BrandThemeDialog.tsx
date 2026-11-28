/** Brand theme on a button press: pick brand images, derive an accessible theme deterministically, compare it with the current design and apply it to the private draft. */
import { useEffect, useMemo, useState } from "react";
import type { RequestFn } from "../../../../frontend/src/shared/api/types";
import {
  Button,
  Checkbox,
  Dialog,
  Field,
  Notice,
  Select,
  useField,
} from "../../../../frontend/src/kit/ui";
import {
  brandSiteFields,
  contrastFindings,
  contrastPairs,
  deriveBrandTheme,
  extractPalette,
  fontPairings,
  logoShape,
  motionIntensities,
  parseBrandTheme,
  suggestions,
  type BrandChoices,
  type BrandPalette,
  type BrandTheme,
  type Density,
  type MotionIntensity,
  type PairingId,
  type RadiusScale,
} from "../../../../frontend/src/kit/tokens";
import type { Theme } from "../model";
import type { SiteProduct } from "../Renderer";
import StorefrontCanvas from "./StorefrontCanvas";
import { acceptedUpload, samplePixels, type BrandSource } from "./brand-images";
import { useBrandText } from "./brand-i18n";
import "./brand-theme.css";

/** The theme fields a brand theme sets on the home page. */
export function brandPatch(brand: BrandTheme, motion: MotionIntensity): Theme {
  return { ...brandSiteFields(brand), motion, brand };
}

export default function BrandThemeDialog({
  request,
  products,
  channel,
  language,
  workspace,
  current,
  sample = samplePixels,
  onApply,
  onClose,
}: {
  request: RequestFn;
  products: SiteProduct[];
  channel: string;
  language: string;
  workspace: string;
  /** The effective appearance of the shop now (inherited and local values). */
  current: Theme;
  /** Image decoding; tests replace it because the test DOM has no canvas. */
  sample?: typeof samplePixels;
  onApply: (theme: Theme) => void;
  onClose: () => void;
}) {
  const t = useBrandText();
  const [logo, setLogo] = useState<BrandSource>();
  const [uploads, setUploads] = useState<BrandSource[]>([]);
  const [chosen, setChosen] = useState<Set<string>>(new Set());
  const [palette, setPalette] = useState<BrandPalette>();
  const [shape, setShape] = useState<RadiusScale>();
  const [choices, setChoices] = useState<BrandChoices>({});
  const [motion, setMotion] = useState<MotionIntensity>(
    current.motion ?? "subtle",
  );
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState("");
  // The company logo is the first source: the public identity names it, the merchant preview serves its pixels.
  useEffect(() => {
    let active = true;
    void request("/store-api/company", undefined, "GET", {
      salesChannel: channel === "*" ? "default" : channel,
    })
      .then(async (v) => {
        const id = v?.data?.logoId as string | undefined;
        if (!id) return;
        const image = await request(`/api/settings/company-logo/${id}`);
        if (!active || !image?.dataUrl) return;
        setLogo({
          id: `logo:${id}`,
          kind: "logo",
          image: image.dataUrl,
          preview: image.dataUrl,
        });
        setChosen((s) => new Set(s).add(`logo:${id}`));
      })
      .catch(() => {});
    return () => {
      active = false;
    };
  }, [request, channel]);
  const productSources = useMemo(
    () =>
      products
        .flatMap((p) =>
          p.media[0]?.url
            ? [
                {
                  id: `media:${p.media[0].id}`,
                  kind: "product" as const,
                  image: p.media[0].url,
                  preview: p.media[0].url,
                },
              ]
            : [],
        )
        .slice(0, 6),
    [products],
  );
  const sources = [...(logo ? [logo] : []), ...uploads, ...productSources];
  const toggle = (id: string) =>
    setChosen((s) => {
      const next = new Set(s);
      if (next.has(id)) next.delete(id);
      else next.add(id);
      return next;
    });
  const upload = (files: FileList | null) => {
    const accepted = [...(files ?? [])].filter(acceptedUpload).slice(0, 4);
    if ((files?.length ?? 0) > accepted.length) setError(t("imageError"));
    const next = accepted.map((file, i) => ({
      id: `upload:${i}:${file.name.slice(0, 80)}`,
      kind: "upload" as const,
      image: file,
      preview: URL.createObjectURL(file),
    }));
    setUploads(next);
    setChosen(
      (s) =>
        new Set([
          ...[...s].filter((id) => !id.startsWith("upload:")),
          ...next.map((n) => n.id),
        ]),
    );
  };
  useEffect(
    () => () => uploads.forEach((u) => URL.revokeObjectURL(u.preview)),
    [uploads],
  );
  const selected = sources.filter((s) => chosen.has(s.id));
  const generate = async () => {
    setError("");
    if (!selected.length) return setError(t("noSources"));
    setBusy(true);
    try {
      const samples = await Promise.all(selected.map((s) => sample(s)));
      const found = extractPalette(samples);
      const logoIndex = selected.findIndex((s) => s.kind !== "product");
      setPalette(found);
      setShape(logoIndex >= 0 ? logoShape(samples[logoIndex]) : undefined);
      const suggested = suggestions(found);
      setChoices({ pairing: suggested.pairing });
      if (!current.motion) setMotion(suggested.motion);
    } catch {
      setError(t("imageError"));
    } finally {
      setBusy(false);
    }
  };
  const brand = useMemo(
    () =>
      palette &&
      deriveBrandTheme(palette, {
        ...choices,
        shape,
        source: selected.map((s) => s.id),
      }),
    [palette, choices, shape, chosen],
  );
  const findings = brand ? contrastFindings(brand.colors) : [];
  const proposed: Theme | undefined = brand && {
    ...current,
    ...brandPatch(brand, motion),
  };
  const apply = () => {
    if (!brand) return;
    // The same validation the server applies on save; a failing document never reaches the draft.
    if ("error" in parseBrandTheme(brand)) return setError(t("contrastFail"));
    onApply(brandPatch(brand, motion));
  };
  const preview = (appearance: Theme, label: string) => (
    <figure className="brand-preview">
      <figcaption>{label}</figcaption>
      <StorefrontCanvas
        request={request}
        workspace={workspace}
        channel={channel}
        language={language}
        device="desktop"
        products={products}
        appearance={appearance}
        previewOnly
        onHome={() => {}}
      />
    </figure>
  );
  return (
    <Dialog
      title={t("brandTitle")}
      description={t("brandIntro")}
      onClose={onClose}
      footer={
        <>
          <Button onClick={onClose}>{t("cancel")}</Button>
          <Button
            tone="primary"
            disabled={!brand || findings.length > 0}
            onClick={apply}
          >
            {t("applyBrand")}
          </Button>
        </>
      }
    >
      <div className="brand-theme">
        <section className="brand-controls">
          <fieldset className="brand-sources">
            <legend>{t("sources")}</legend>
            {sources.map((s) => (
              <span className="brand-source" key={s.id}>
                <img src={s.preview} alt="" />
                <Checkbox
                  label={
                    s.kind === "logo"
                      ? t("companyLogo")
                      : s.kind === "upload"
                        ? t("uploadedImage")
                        : t("productImage")
                  }
                  checked={chosen.has(s.id)}
                  onChange={() => toggle(s.id)}
                />
              </span>
            ))}
          </fieldset>
          <Field label={t("uploadImages")} hint={t("uploadHint")}>
            <UploadInput onFiles={upload} />
          </Field>
          <Button tone="primary" busy={busy} onClick={() => void generate()}>
            {t("generate")}
          </Button>
          {error && <Notice tone="danger">{error}</Notice>}
          {brand && (
            <>
              <div className="brand-palette" aria-label={t("palette")}>
                {[
                  brand.palette.primary,
                  brand.palette.secondary,
                  brand.colors.accent,
                  brand.colors.text,
                  brand.colors.bg,
                ]
                  .filter((c): c is string => !!c)
                  .map((c, i) => (
                    <span key={i} style={{ background: c }} title={c} />
                  ))}
              </div>
              <Notice tone={findings.length ? "danger" : "positive"}>
                {findings.length
                  ? t("contrastFail")
                  : t("contrastOk").replace(
                      "{count}",
                      String(contrastPairs.length),
                    )}
              </Notice>
              <Field label={t("typography")}>
                <Select
                  value={brand.fonts.pairing}
                  onChange={(event) =>
                    setChoices({
                      ...choices,
                      pairing: event.target.value as PairingId,
                    })
                  }
                >
                  {(Object.keys(fontPairings) as PairingId[]).map((p) => (
                    <option key={p} value={p}>
                      {t(p)}
                    </option>
                  ))}
                </Select>
              </Field>
              <Field label={t("corners")}>
                <Select
                  value={brand.radius}
                  onChange={(event) =>
                    setChoices({
                      ...choices,
                      radius: event.target.value as RadiusScale,
                    })
                  }
                >
                  {(["sharp", "soft", "round"] as const).map((r) => (
                    <option key={r} value={r}>
                      {t(r)}
                    </option>
                  ))}
                </Select>
              </Field>
              <Field label={t("spacing")}>
                <Select
                  value={brand.density}
                  onChange={(event) =>
                    setChoices({
                      ...choices,
                      density: event.target.value as Density,
                    })
                  }
                >
                  {(["compact", "regular", "airy"] as const).map((r) => (
                    <option key={r} value={r}>
                      {t(r)}
                    </option>
                  ))}
                </Select>
              </Field>
              <Field label={t("motion")} hint={t("motionHint")}>
                <Select
                  value={motion}
                  onChange={(event) =>
                    setMotion(event.target.value as MotionIntensity)
                  }
                >
                  {motionIntensities.map((m) => (
                    <option key={m} value={m}>
                      {t(m)}
                    </option>
                  ))}
                </Select>
              </Field>
              <p className="brand-note">{t("applyNote")}</p>
            </>
          )}
        </section>
        <div className="brand-compare">
          {preview(current, t("currentDesign"))}
          {proposed && preview(proposed, t("brandDesign"))}
        </div>
      </div>
    </Dialog>
  );
}

/** The local file picker, labelled by its Field. */
function UploadInput({
  onFiles,
}: {
  onFiles: (files: FileList | null) => void;
}) {
  const field = useField();
  return (
    <input
      id={field.id}
      aria-describedby={field.describedBy}
      type="file"
      accept="image/png,image/jpeg,image/webp"
      multiple
      onChange={(event) => onFiles(event.target.files)}
    />
  );
}
