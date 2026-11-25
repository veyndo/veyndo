/** One merchant entry for inbound shops and native agent checkout; technical support never masquerades as external approval. */
import { Button, Card, Field, Notice, Select } from "../../kit/ui";
import { useEffect, useState } from "react";
import type { RequestFn } from "../shell/studio-types";
import type { IntakeMode } from "../catalog/catalog-intake-model";
import type { Channel } from "./channel-model";
import { useLocale } from "../../shared/i18n/i18n";
import { contentText } from "../../shared/i18n/content-language";
import Icon from "../../shared/ui/Icon";
import { useAgenticChannelText } from "./agentic-channel-i18n";
import "./agentic-channels.css";
type Readiness = {
  cartTransfer: boolean;
  buyerConsent: boolean;
  poppy?: { published: boolean; url?: string | null; version: string };
  signedOffers?: {
    configured: boolean;
    keyId?: string | null;
    validSeconds: number;
    offerPath: string;
    verifyPath: string;
    keysPath: string;
  };
  externalAcceptance: string;
  configuredPayments: { id: string; mode: string }[];
};
export default function AgenticChannels({
  request,
  canRead,
  canImport,
  onImport,
  onBack,
  channels,
  mainLocale,
  selectedChannel,
  onChannel,
}: {
  request: RequestFn;
  canRead: boolean;
  canImport: boolean;
  onImport: (mode: IntakeMode) => void;
  onBack: () => void;
  channels: Channel[];
  mainLocale: string;
  selectedChannel?: string;
  onChannel?: (channel: string) => void;
}) {
  const t = useAgenticChannelText(),
    { locale } = useLocale();
  const [localChannel, setLocalChannel] = useState("default"),
    [readiness, setReadiness] = useState<Readiness>(),
    [error, setError] = useState("");
  const channel = selectedChannel ?? localChannel;
  const setChannel = onChannel ?? setLocalChannel;
  useEffect(() => {
    let active = true;
    setReadiness(undefined);
    setError("");
    if (canRead)
      request("/api/merchant/catalog/intake", undefined, undefined, {
        salesChannel: channel,
      })
        .then((v) => {
          if (active) setReadiness(v.agentic);
        })
        .catch((e) => {
          if (active) setError(e.message);
        });
    return () => {
      active = false;
    };
  }, [request, channel, canRead]);
  const cards = [
    {
      id: "shopify",
      icon: "code",
      hint: "shopifyHint",
      label: "importShopify",
      mode: "shopify_csv",
    },
    {
      id: "supplier",
      icon: "globe",
      hint: "supplierHint",
      label: "importSupplier",
      mode: "shop_mcp",
    },
    {
      id: "others",
      icon: "link",
      hint: "othersHint",
      label: "csv",
      mode: "csv",
    },
  ] as const;
  return (
    <section className="agentic-channels">
      <header>
        <div>
          <Button type="button" onClick={onBack}>
            <Icon name="arrow" />
            {t("back")}
          </Button>
          <h2>{t("title")}</h2>
          <p>{t("intro")}</p>
        </div>
        <Field label={t("channel")}>
          <Select value={channel} onChange={(e) => setChannel(e.target.value)}>
            {channels.length ? (
              channels.map((c) => (
                <option key={c.id} value={c.id}>
                  {contentText(c.data.name, locale, mainLocale) || c.id}
                </option>
              ))
            ) : (
              <option value="default">{t("default")}</option>
            )}
          </Select>
        </Field>
      </header>
      {!canRead && <p role="status">{t("permission")}</p>}
      {error && <Notice tone="danger">{error}</Notice>}
      <Card as="article" className="agentic-protocol">
        <div>
          <Icon name="spark" />
          <h3>{t("google")}</h3>
          <p>{t("googleHint")}</p>
          <a
            href="https://developers.google.com/merchant/ucp/guides/overview/cart-overview"
            target="_blank"
            rel="noreferrer"
          >
            {t("guide")} ↗
          </a>
        </div>
        <div>
          <h4>{t("native")}</h4>
          <dl>
            <dt>{t("cart")}</dt>
            <dd>{t(readiness?.cartTransfer ? "ready" : "unknown")}</dd>
            <dt>{t("consent")}</dt>
            <dd>{t(readiness?.buyerConsent ? "ready" : "unknown")}</dd>
          </dl>
          {readiness && <p className="soft-tag">{t("approval")}</p>}
        </div>
      </Card>
      {readiness?.poppy && (
        <Card as="article" className="agentic-protocol">
          <div>
            <Icon name="agents" />
            <h3>{t("poppyTitle")}</h3>
            <p>{t("poppyHint")}</p>
            <p>{t("poppyScope")}</p>
          </div>
          <div>
            {readiness.poppy.published ? (
              <dl>
                <dt>{t("poppyTitle").split(":")[0]}</dt>
                <dd>{t("poppyPublished")}</dd>
                <dt>{t("poppyVersion")}</dt>
                <dd>{readiness.poppy.version}</dd>
                <dt>{t("poppyFile")}</dt>
                <dd>
                  <code>{readiness.poppy.url}</code>
                </dd>
              </dl>
            ) : (
              <p role="status">{t("poppyMissing")}</p>
            )}
          </div>
        </Card>
      )}
      {readiness?.signedOffers && (
        <Card as="article" className="agentic-protocol">
          <div>
            <Icon name="lock" />
            <h3>{t("offerTitle")}</h3>
            <p>{t("offerHint")}</p>
            <p>{t("offerScope")}</p>
          </div>
          <div>
            {readiness.signedOffers.configured ? (
              <dl>
                <dt>{t("offerTitle")}</dt>
                <dd>{t("offerActive")}</dd>
                <dt>{t("offerKey")}</dt>
                <dd>
                  <code>{readiness.signedOffers.keyId?.slice(0, 16)}</code>
                </dd>
                <dt>{t("offerValid")}</dt>
                <dd>
                  {t("offerMinutes").replace(
                    "{minutes}",
                    String(readiness.signedOffers.validSeconds / 60),
                  )}
                </dd>
                <dt>{t("offerPaths")}</dt>
                <dd>
                  <code>{readiness.signedOffers.offerPath}</code>
                  <br />
                  <code>{readiness.signedOffers.verifyPath}</code>
                  <br />
                  <code>{readiness.signedOffers.keysPath}</code>
                </dd>
              </dl>
            ) : (
              <p role="status">{t("offerMissing")}</p>
            )}
          </div>
        </Card>
      )}
      <div className="agentic-import-cards">
        {cards.map((c) => (
          <Card as="article" key={c.id}>
            <span className="channel-avatar">
              <Icon name={c.icon} size={24} />
            </span>
            <h3>{t(c.id)}</h3>
            <p>{t(c.hint)}</p>
            <Button
              tone="primary"
              type="button"
              disabled={!canRead || !canImport}
              onClick={() => onImport(c.mode)}
            >
              {t(c.label)}
            </Button>
            {c.id === "others" && (
              <Button
                type="button"
                disabled={!canRead || !canImport}
                onClick={() => onImport("url")}
              >
                {t("url")}
              </Button>
            )}
          </Card>
        ))}
      </div>
      <Card as="aside">
        <h3>{t("payment")}</h3>
        <p>{t("paymentHint")}</p>
        <div>
          {readiness?.configuredPayments.map((p) => (
            <span className="soft-tag" key={p.id}>
              {p.id} · {p.mode}
            </span>
          ))}
        </div>
      </Card>
    </section>
  );
}
