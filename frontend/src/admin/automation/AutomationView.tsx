/** Typed merchant rule/campaign/flow/channel forms with exact JSON available for advanced review. */
import { Button, Card, Notice } from "../../kit/ui";
import { useGoalTemplate } from "./goal-template";
import { useAutomationAccess } from "./useAutomationAccess";
import AutomationDefinitions from "./AutomationDefinitions";
import { useAutomationLanguages } from "./useAutomationLanguages";
import AutomationDelete from "./AutomationDelete";
import { useLifecycleText } from "./lifecycle-i18n";
import "./automation-lifecycle.css";
import EntityHistory from "../../shared/history/EntityHistory";
import { ContentLanguage } from "../../shared/i18n/ContentLanguage";
import "../settings/international.css";
import FlowExecution from "./FlowExecution";
import { useConnectedText } from "../../shared/i18n/connected-i18n";
import { type Config, type Kind } from "./automation-types";
import AutomationEditor from "./AutomationEditor";
import { type AutomationCatalog } from "./RuleBuilder";

import { useEffect, useMemo, useRef, useState } from "react";

import { useWorkbenchText } from "../../shared/i18n/workbench-i18n";
import FlowProposalReview from "./FlowProposalReview";
import type { RequestFn } from "../shell/studio-types";
import { useInvalidateRead, useRead, useSetRead } from "../shell/useRead";
/** The built-in catalogue shown until the server catalogue arrives. */
const defaultCatalog: AutomationCatalog = {
  conditions: [
    "andContainer",
    "orContainer",
    "notContainer",
    "alwaysValid",
    "cartCartAmount",
    "cartLineItemCount",
    "customerGroup",
    "shippingCountry",
    "salesChannel",
    "lineItemId",
    "customerLoggedIn",
    "orderState",
    "paymentState",
    "deliveryState",
    "contextField",
    "eventField",
  ],
  fields: [],
  events: ["order.placed"],
  apps: [],
};
const noRows: Record<Kind, Config[]> = {
  rules: [],
  promotions: [],
  flows: [],
  channels: [],
};
export default function AutomationView({
  request,
  onChannels,
  onTranslations,
}: {
  request: RequestFn;
  role: string;
  onChannels?: () => void;
  onTranslations?: (jobId: string) => void;
}) {
  const t = useLifecycleText();
  const { x } = useConnectedText();
  const [actionError, setError] = useState("");
  const languages = useAutomationLanguages(request, setError);
  const [contentLanguage, setContentLanguage] = useState("");
  const [imported, setImported] = useState("");
  const { w, locale } = useWorkbenchText();
  const [kind, setKind] = useState<Kind>("rules");
  const [id, setId] = useState("");
  const [revision, setRevision] = useState(0);
  const [data, setData] = useState<Record<string, any>>({
    name: {},
    active: true,
    condition: { type: "cartCartAmount", operator: ">=", amount: 100 },
  });
  const goal = useGoalTemplate(
    languages,
    setKind,
    setId,
    setRevision,
    setData,
    setContentLanguage,
  );
  const [busy, setBusy] = useState(false);
  const [advanced, setAdvanced] = useState("");
  const { manage, canCompare } = useAutomationAccess(request);
  const choose = (r: Config, k: Kind = kind) => {
    setKind(k);
    setId(r.id);
    setRevision(r.revision);
    setData(r.data);
    setAdvanced("");
    setError("");
  };
  const catalogRead = useRead<AutomationCatalog>(
    request,
    "/api/automation/catalog",
  );
  const automation = useRead(request, "/api/automation");
  // Execution states poll every three seconds while the view is open.
  const executions = useRead(request, "/api/automation/executions", {
    refetchMs: 3000,
  });
  const setRead = useSetRead(request);
  const invalidate = useInvalidateRead(request);
  const rows: Record<Kind, Config[]> = automation.data ?? noRows;
  const jobs: any[] = executions.data?.jobs ?? automation.data?.jobs ?? [];
  const catalog: AutomationCatalog = useMemo(() => {
    const base = catalogRead.data ?? defaultCatalog;
    return automation.data
      ? {
          ...base,
          rules: automation.data.rules.map((r: Config) => ({
            id: r.id,
            name: r.data.name,
            revision: r.revision,
          })),
        }
      : base;
  }, [catalogRead.data, automation.data]);
  const error =
    actionError || (automation.error as Error | undefined)?.message || "";
  const load = async () => {
    setRead("/api/automation", await request("/api/automation"));
    invalidate("/api/automation/executions");
  };
  // The first rule opens once the list arrives, unless a goal template chose.
  const opened = useRef(false);
  useEffect(() => {
    const v = automation.data;
    if (!v || opened.current) return;
    opened.current = true;
    if (!goal.current && v.rules[0]) choose(v.rules[0], "rules");
  }, [automation.data]);
  const fresh = (k: Kind) => {
    setKind(k);
    setContentLanguage(languages?.mainLocale ?? "en-GB");
    setId("");
    setRevision(0);
    setAdvanced("");
    const common = { name: {}, active: true };
    setData(
      k === "rules"
        ? {
            ...common,
            condition: { type: "cartCartAmount", operator: ">=", amount: 100 },
          }
        : k === "promotions"
          ? {
              ...common,
              code: null,
              kind: "percentage",
              amount: 10,
              rule: { type: "alwaysValid" },
              exclusive: false,
              priority: 0,
              maxUses: null,
              start: null,
              end: null,
            }
          : k === "flows"
            ? {
                ...common,
                event: "order.placed",
                condition: { type: "alwaysValid" },
                action: "note",
                instruction: {},
                locale: languages?.mainLocale ?? "en-GB",
                inference: { provider: "ollama" },
              }
            : {
                ...common,
                kind: "storefront",
                locales: languages?.locales ?? ["en-GB"],
                productIds: [],
              },
    );
  };
  const update = (key: string, value: unknown) => {
    setAdvanced("");
    setData((d) => ({ ...d, [key]: value }));
  };
  const run = async (fn: () => Promise<void>, reload = true) => {
    setBusy(true);
    setError("");
    try {
      await fn();
      if (reload) await load();
    } catch (e) {
      setError((e as Error).message);
    } finally {
      setBusy(false);
    }
  };
  return (
    <ContentLanguage
      key={
        languages
          ? `${languages.mainLocale}:${languages.locales.join(",")}`
          : "loading"
      }
      locales={languages?.locales ?? ["en-GB"]}
      mainLocale={languages?.mainLocale ?? "en-GB"}
      language={contentLanguage || languages?.mainLocale}
      onLanguageChange={setContentLanguage}
    >
      <div className="studio-page workbench">
        <div className="page-intro">
          <span className="kicker">{w("automation")}</span>
          <h1>{w("automation")}</h1>
          <p>{w("automationHint")}</p>
        </div>
        <nav className="workbench-row">
          {(["rules", "promotions", "flows", "channels"] as Kind[]).map((k) => (
            <Button
              tone={kind === k ? "primary" : "secondary"}
              type="submit"
              key={k}
              onClick={() =>
                k === "channels" && onChannels
                  ? onChannels()
                  : rows[k][0]
                    ? choose(rows[k][0], k)
                    : fresh(k)
              }
            >
              {w(k)}
            </Button>
          ))}
        </nav>
        <aside className="automation-process-guide">
          <h2>{t("processes")}</h2>
          <p>{t("processHint")}</p>
          <p className="muted">{t("coreHint")}</p>
        </aside>
        <div
          className={`workbench-grid ${kind === "flows" ? "flow-workspace" : ""}`}
        >
          <AutomationDefinitions
            rows={rows[kind]}
            id={id}
            mainLocale={languages?.mainLocale ?? "en-GB"}
            manage={manage}
            onCreate={() => fresh(kind)}
            onSelect={(r) => choose(r)}
          />
          <Card>
            <AutomationEditor
              canCompare={canCompare}
              run={run}
              request={request}
              kind={kind}
              id={id}
              advanced={advanced}
              data={data}
              setRevision={setRevision}
              w={w}
              setId={setId}
              revision={revision}
              update={update}
              x={x}
              imported={imported}
              setImported={setImported}
              busy={busy || !languages}
              manage={manage}
              catalog={catalog}
              rows={rows}
              locale={locale}
              setAdvanced={setAdvanced}
            />
            <AutomationDelete
              key={`${kind}:${id}`}
              request={request}
              kind={kind}
              id={id}
              revision={revision}
              disabled={
                !manage ||
                busy ||
                JSON.stringify(data) !==
                  JSON.stringify(rows[kind].find((r) => r.id === id)?.data)
              }
              onReference={(k, i) => {
                const r = rows[k].find((r) => r.id === i);
                if (r) choose(r, k);
              }}
              onDeleted={async () => {
                await load();
                fresh(kind);
              }}
            />
            {!!id && revision > 0 && (
              <EntityHistory
                request={request}
                entity={
                  {
                    rules: "rule",
                    flows: "flow",
                    promotions: "promotion",
                    channels: "channel",
                  }[kind]
                }
                id={id}
                revision={revision}
                dirty={
                  busy ||
                  JSON.stringify(data) !==
                    JSON.stringify(rows[kind].find((r) => r.id === id)?.data)
                }
                onRestored={async () => {
                  const next = await request("/api/automation");
                  setRead("/api/automation", next);
                  const record = next[kind].find((r: Config) => r.id === id);
                  if (record) {
                    setData(record.data);
                    setRevision(record.revision);
                    setAdvanced("");
                  }
                }}
              />
            )}
          </Card>
        </div>
        {error && <Notice tone="danger">{error}</Notice>}
        <Card>
          <h2>{w("executions")}</h2>
          {jobs.map((j) => (
            <article key={j.id}>
              <strong>{j.flow}</strong>
              <p>
                {w(
                  [
                    "queued",
                    "running",
                    "completed",
                    "failed",
                    "uncertain",
                  ].includes(j.state)
                    ? (j.state as "queued")
                    : "failed",
                )}
              </p>
              {j.result?.note && <p>{j.result.note}</p>}
              <FlowExecution job={j} />
              {j.result?.preview && (
                <FlowProposalReview
                  taskId={j.result.taskId}
                  preview={j.result.preview}
                  applied={!!j.applied}
                  executionStarted={!!j.executionStarted}
                  request={request}
                  manage={manage}
                  onTranslations={onTranslations}
                />
              )}{" "}
              {j.error && <p>{j.error}</p>}
            </article>
          ))}
        </Card>
        <p className="muted">{w("nativeSubset")}</p>
      </div>
    </ContentLanguage>
  );
}
