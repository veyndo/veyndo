/** Supplier links in the native settings revision and reorder proposals with signed supplier offers; approval hands over to the supplier checkout. */
import { Button, Card, Field, Notice, TextInput } from "../../kit/ui";
import { useState } from "react";
import type { Config } from "../../shared/api/shop-api";
import type { RequestFn } from "../shell/studio-types";
import { useSettingsDraft } from "../settings/useSettingsDraft";
import { useLocale } from "../../shared/i18n/i18n";
import Icon from "../../shared/ui/Icon";
import { useSupplyText } from "./supply-i18n";
import "./supply.css";
type Link = {
  productId: string;
  supplierShop: string;
  supplierProductId: string;
  reorderPoint: number;
  reorderQuantity: number;
};
type Supply = { maxOrderMinor?: number | null; links: Link[] };
type Settings = Config & { supply?: Supply };
type Line = {
  productId: string;
  name?: string;
  stock?: number;
  reorderPoint?: number;
  quantity?: number;
  supplierShop: string;
  error?: string;
  detail?: string;
  sameCurrency?: boolean;
  withinCeiling?: boolean;
  offer?: {
    cartToken: string;
    payloadBase64: string;
    signature: string;
    supplierStockEnough: boolean;
    payload: {
      totalMinor: string;
      currency: string;
      currencyScale: number;
      expiresAt: number;
    };
  };
};
type Proposal = { lines: Line[]; sufficient: number };
const fields = [
  ["productId", "product"],
  ["supplierShop", "supplier"],
  ["supplierProductId", "supplierProduct"],
] as const;
export default function SupplyReorder({
  request,
  rights,
}: {
  request: RequestFn;
  rights: string[];
}) {
  const t = useSupplyText(),
    { locale } = useLocale();
  const state = useSettingsDraft<Settings>(request, "/api/merchant/commerce");
  const [proposal, setProposal] = useState<Proposal>(),
    [busy, setBusy] = useState(false),
    [error, setError] = useState(""),
    [links, setLinks] = useState<Record<string, string>>({});
  const canWrite = rights.includes("settings.write");
  if (!rights.includes("settings.read")) return null;
  const settings = state.value?.data;
  const supply: Supply = settings?.supply ?? { links: [] };
  const currency = settings?.currencies?.pricingCurrency ?? "";
  const scale =
    settings?.currencies?.definitions.find((c) => c.code === currency)?.scale ??
    2;
  const update = (next: Supply) => {
    if (settings) state.change({ ...settings, supply: next });
  };
  const money = (minor: number, code: string, digits: number) => {
    try {
      return new Intl.NumberFormat(locale, {
        style: "currency",
        currency: code,
      }).format(minor / 10 ** digits);
    } catch {
      return `${(minor / 10 ** digits).toFixed(digits)} ${code}`;
    }
  };
  const run = async (work: () => Promise<void>) => {
    setBusy(true);
    setError("");
    try {
      await work();
    } catch (e) {
      setError((e as Error).message);
    } finally {
      setBusy(false);
    }
  };
  const propose = () =>
    run(async () => {
      setLinks({});
      setProposal(
        (await request(
          "/api/merchant/supply/proposal",
          {},
          "POST",
        )) as Proposal,
      );
    });
  const approve = (line: Line) =>
    run(async () => {
      const done = (await request(
        "/api/merchant/supply/approve",
        {
          productId: line.productId,
          payloadBase64: line.offer!.payloadBase64,
          signature: line.offer!.signature,
          cartToken: line.offer!.cartToken,
          approve: true,
        },
        "POST",
      )) as { checkoutPath: string };
      setLinks((old) => ({ ...old, [line.productId]: done.checkoutPath }));
    });
  return (
    <Card as="article" className="supply-reorder">
      <header>
        <Icon name="box" />
        <div>
          <h3>{t("title")}</h3>
          <p>{t("intro")}</p>
          <p className="soft-tag">{t("scope")}</p>
        </div>
      </header>
      {(state.error || error) && (
        <Notice tone="danger">{state.error || error}</Notice>
      )}
      {!canWrite && <p role="status">{t("readOnly")}</p>}
      {settings && (
        <fieldset disabled={!canWrite || state.busy || busy}>
          <label className="supply-ceiling">
            {t("ceiling", { currency })}
            <input
              type="number"
              min={0}
              step={1 / 10 ** scale}
              value={
                supply.maxOrderMinor == null
                  ? ""
                  : supply.maxOrderMinor / 10 ** scale
              }
              onChange={(e) =>
                update({
                  ...supply,
                  maxOrderMinor:
                    e.target.value === ""
                      ? null
                      : Math.round(Number(e.target.value) * 10 ** scale),
                })
              }
            />
            <small>{t("ceilingHint")}</small>
          </label>
          {supply.links.map((link, index) => {
            const change = (patch: Partial<Link>) =>
              update({
                ...supply,
                links: supply.links.map((row, i) =>
                  i === index ? { ...row, ...patch } : row,
                ),
              });
            return (
              <div className="supply-link" key={index}>
                {fields.map(([field, word]) => (
                  <label key={field}>
                    {t(word)}
                    <input
                      required
                      maxLength={160}
                      value={link[field]}
                      onChange={(e) => change({ [field]: e.target.value })}
                    />
                  </label>
                ))}
                <Field label={t("point")}>
                  <TextInput
                    type="number"
                    min={0}
                    max={100000}
                    value={link.reorderPoint}
                    onChange={(e) =>
                      change({ reorderPoint: Number(e.target.value) })
                    }
                  />
                </Field>
                <Field label={t("quantity")}>
                  <TextInput
                    type="number"
                    min={1}
                    max={1000}
                    value={link.reorderQuantity}
                    onChange={(e) =>
                      change({ reorderQuantity: Number(e.target.value) })
                    }
                  />
                </Field>
                <Button
                  type="button"
                  onClick={() =>
                    update({
                      ...supply,
                      links: supply.links.filter((_, i) => i !== index),
                    })
                  }
                >
                  {t("remove")}
                </Button>
              </div>
            );
          })}
          <div className="studio-actions">
            <Button
              type="button"
              disabled={supply.links.length >= 200}
              onClick={() =>
                update({
                  ...supply,
                  links: [
                    ...supply.links,
                    {
                      productId: "",
                      supplierShop: "",
                      supplierProductId: "",
                      reorderPoint: 5,
                      reorderQuantity: 10,
                    },
                  ],
                })
              }
            >
              {t("add")}
            </Button>
            <Button
              tone="primary"
              type="button"
              disabled={!state.dirty}
              onClick={() => void state.save()}
            >
              {t("save")}
            </Button>
          </div>
        </fieldset>
      )}
      <div className="studio-actions">
        <Button
          type="button"
          disabled={busy || state.dirty || !supply.links.length}
          onClick={() => void propose()}
        >
          {t("propose")}
        </Button>
        {state.dirty && <small>{t("saveFirst")}</small>}
      </div>
      {proposal && !proposal.lines.length && (
        <p role="status">{t("none", { count: proposal.sufficient })}</p>
      )}
      {proposal?.lines.map((line) => (
        <div className="supply-line" key={line.productId}>
          <strong>{line.name ?? line.productId}</strong>
          {line.error === "product_missing" && <p>{t("missing")}</p>}
          {line.error === "supplier_refused" && (
            <p>
              {t("refused")} <q>{line.detail}</q>
            </p>
          )}
          {!line.error && (
            <p>
              {t("line", {
                stock: line.stock ?? 0,
                point: line.reorderPoint ?? 0,
                quantity: line.quantity ?? 0,
                supplier: line.supplierShop,
              })}
            </p>
          )}
          {line.offer && (
            <>
              <p>
                {t("offer", {
                  total: money(
                    Number(line.offer.payload.totalMinor),
                    line.offer.payload.currency,
                    line.offer.payload.currencyScale,
                  ),
                  time: new Date(
                    line.offer.payload.expiresAt * 1000,
                  ).toLocaleTimeString(locale),
                })}{" "}
                <span
                  className={`soft-tag ${line.withinCeiling ? "is-ok" : "is-warn"}`}
                >
                  {t(line.withinCeiling ? "within" : "over")}
                </span>
              </p>
              {line.sameCurrency === false && <p>{t("otherCurrency")}</p>}
              {!line.offer.supplierStockEnough && <p>{t("lowStock")}</p>}
              {links[line.productId] ? (
                <p role="status">
                  {t("approved")}{" "}
                  <a
                    href={links[line.productId]}
                    target="_blank"
                    rel="noreferrer"
                  >
                    {t("open")} ↗
                  </a>
                </p>
              ) : (
                <Button
                  tone="primary"
                  type="button"
                  disabled={busy || !canWrite || !line.withinCeiling}
                  onClick={() => void approve(line)}
                >
                  {t("approve")}
                </Button>
              )}
            </>
          )}
        </div>
      ))}
    </Card>
  );
}
