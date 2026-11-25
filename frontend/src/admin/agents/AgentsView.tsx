/** AgentsView renders verified shop state and typed user actions. */
import { Button, Card, Notice } from "../../kit/ui";
import { useState } from "react";
import { useLocale } from "../../shared/i18n/i18n";
import Icon from "../../shared/ui/Icon";
import { BRAND } from "../../shared/ui/Brand";
import type { Overview } from "../shell/studio-types";
const guide = `${BRAND.repository}/blob/main/docs/connectors.md`;
export function AgentsView({
  data,
  onStorefront,
}: {
  data: Overview;
  onStorefront: () => void;
}) {
  const { t, number } = useLocale();
  const [copied, setCopied] = useState(false);
  const [copyFailed, setCopyFailed] = useState(false);
  return (
    <div className="studio-page">
      <div className="page-intro">
        <span className="kicker">MCP / UCP</span>
        <h1>{t("agentTitle")}</h1>
        <p>{t("agentSub")}</p>
      </div>
      <section className="agent-journey">
        {[
          ["chat", "stepAsk", "stepAskText"],
          ["graph", "stepDiscover", "stepDiscoverText"],
          ["box", "stepCheckout", "stepCheckoutText"],
        ].map(([icon, title, body], i) => (
          <div key={title}>
            <span className="journey-icon">
              <Icon name={icon as "chat" | "graph" | "box"} />
              <small>0{i + 1}</small>
            </span>
            <h2>{t(title as "stepAsk")}</h2>
            <p>{t(body as "stepAskText")}</p>
          </div>
        ))}
      </section>
      <div className="connection-grid">
        {["ChatGPT", "Claude"].map((name) => (
          <Card className="connection-card" key={name}>
            <div className="provider-symbol">{name.slice(0, 1)}</div>
            <div>
              <h2>{name}</h2>
              <span className="soft-tag pending">{t("notLinked")}</span>
            </div>
            <p>{t("remoteHint")}</p>
            <a
              href={guide}
              target="_blank"
              rel="noreferrer"
              className="studio-secondary"
            >
              {t("setupGuide")}
              <Icon name="arrow" size={16} />
            </a>
          </Card>
        ))}
        <Card className="connection-card">
          <div className="provider-symbol">
            <Icon name="link" />
          </div>
          <div>
            <h2>Claude Desktop / MCP</h2>
            <span className="soft-tag positive">{t("readyLocal")}</span>
          </div>
          <p>{t("mcpDesc")}</p>
          <Button
            type="submit"
            onClick={async () => {
              try {
                await navigator.clipboard.writeText(
                  JSON.stringify(data.connections.localMCPConfig, null, 2),
                );
                setCopied(true);
                setCopyFailed(false);
              } catch {
                setCopyFailed(true);
              }
            }}
          >
            {copied ? t("copied") : t("copyConfig")}
            <Icon name={copied ? "check" : "copy"} size={16} />
          </Button>
          {copyFailed && <Notice tone="danger">{t("copyFailed")}</Notice>}
        </Card>
      </div>
      <div className="overview-columns">
        <Card>
          <div className="card-heading">
            <h2>{t("channelCalls")}</h2>
            <span className="soft-tag">{t("api")}</span>
          </div>
          {["storefront", "mcp", "ucp"].map((name) => (
            <div className="channel-stat" key={name}>
              <span>
                {name === "storefront" ? t("stores") : name.toUpperCase()}
              </span>
              <strong>
                {number(
                  data.channels.find((c) => c.channel === name)?.calls || 0,
                )}
              </strong>
            </div>
          ))}
          <p className="muted">{t("channelHint")}</p>
        </Card>
        <Card>
          <div className="card-heading">
            <h2>UCP · {t("orders")}</h2>
            <span className="soft-tag positive">{t("readyLocal")}</span>
          </div>
          <p>{t("ucpDesc")}</p>
          <div className="protocol-path">
            {t("products")}
            <Icon name="arrow" size={14} />
            {t("quantity")}
            <Icon name="arrow" size={14} />
            {t("orders")}
          </div>
          <Button tone="primary" type="submit" onClick={onStorefront}>
            {t("checkoutDemo")}
            <Icon name="arrow" size={16} />
          </Button>
        </Card>
      </div>
      <Card className="explanation-card">
        <Icon name="spark" />
        <div>
          <h2>{t("modelVsChannel")}</h2>
          <p>{t("modelVsChannelText")}</p>
        </div>
      </Card>
    </div>
  );
}
