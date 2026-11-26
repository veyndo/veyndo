/** Read-only sovereign AI report: whether the mode is on and which host every model route uses; the operator owns the switch. */
import { useRead } from "../../../../../frontend/src/admin/shell/useRead";
import type { RequestFn } from "../../../../../frontend/src/admin/shell/studio-types";
import Icon from "../../../../../frontend/src/shared/ui/Icon";
import { useSovereignText, type SovereignWord } from "./sovereign-i18n";
import "./sovereign.css";
export type Sovereignty = {
  enabled: boolean;
  lockedByDeployment: boolean;
  holds: boolean;
  refusedProviders: string[];
  routes: {
    use: string;
    configured: boolean;
    host: string | null;
    selfHosted: boolean | null;
    admitted: boolean;
  }[];
};
const uses = ["text", "embeddings", "reranker", "images"];
const providerNames: Record<string, string> = {
  openai: "OpenAI",
  anthropic: "Claude",
};
export default function SovereignAI({ request }: { request: RequestFn }) {
  const t = useSovereignText();
  const report = (
    useRead(request, "/api/agent/providers").data as
      { sovereignty?: Sovereignty } | undefined
  )?.sovereignty;
  const value = Array.isArray(report?.routes) ? report : undefined;
  // Without the assistant permission there is nothing to report; the card stays out of the way.
  if (!value) return null;
  const state = value.enabled ? (value.holds ? "on" : "onBroken") : "off";
  const describe = (route: Sovereignty["routes"][number]) => {
    if (route.use === "images" && value.enabled) return t("imagesRefused");
    if (!route.configured || !route.host) return t("unused");
    if (route.selfHosted) return t("own", { host: route.host });
    return t(route.admitted ? "foreign" : "refused", { host: route.host });
  };
  return (
    <section className="sovereign" data-state={state}>
      <header>
        <span className="sovereign-emblem">
          <Icon name="lock" size={20} />
        </span>
        <div>
          <h3>{t("title")}</h3>
          <p>{t(state)}</p>
        </div>
      </header>
      <ul>
        {value.routes
          .filter((route) => uses.includes(route.use))
          .map((route) => (
            <li
              key={route.use}
              data-own={route.selfHosted === true}
              data-refused={
                value.enabled && route.configured && !route.admitted
              }
            >
              <strong>{t(route.use as SovereignWord)}</strong>
              <span>{describe(route)}</span>
            </li>
          ))}
      </ul>
      {value.refusedProviders.length > 0 && (
        <p>
          {t("providersRefused", {
            list: value.refusedProviders
              .map((id) => providerNames[id] ?? id)
              .join(", "),
          })}
        </p>
      )}
      {value.lockedByDeployment && <p>{t("locked")}</p>}
      <p className="sovereign-limits">{t("limits")}</p>
    </section>
  );
}
