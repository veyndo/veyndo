/** Immutable experiment designs and lifecycle controls share the core HTTP/MCP owner and current settings rights. */
import { useRead, useSetRead } from "../shell/useRead";
import { Button, Field, Notice, Select, TextInput } from "../../kit/ui";
import { useEffect, useState } from "react";
import type { Config } from "../../shared/api/shop-api";
import type { RequestFn } from "../shell/studio-types";
import { useKnowledgeText } from "../../shared/i18n/knowledge-i18n";
import { useLocale } from "../../shared/i18n/i18n";
import { useStudio } from "../shell/StudioContext";
import {
  AutomationFields,
  AutomationNote,
  ExperimentSuggestion,
  type Automation,
} from "./ExperimentAutomation";
type Design = {
  title: string;
  locale: string;
  channel: string;
  currency: string;
  control: string;
  treatment: string;
  durationHours: number;
  settlementDays: number;
  minimumPerArm: number;
  outcomeCapMinor: number;
  cupedTheta: number;
  automation?: Automation;
};
type Experiment = {
  id: string;
  design: Design;
  revision: number;
  state: string;
  endsAt?: string;
};
type Readout = {
  causalUpliftProven: boolean;
  unitsPerArm: number[];
  netCollectedMinor: string;
  currency: string;
  interval95?: number[] | null;
  decision?: string;
};
export default function ExperimentStudio({ request }: { request: RequestFn }) {
  const k = useKnowledgeText(),
    { locale, date } = useLocale(),
    { access } = useStudio();
  const experiments = useRead(request, "/api/intelligence/experiments", {
    body: {},
    method: "POST",
  });
  const automation = useRead(request, "/api/automation");
  const commerce = useRead(request, "/api/merchant/commerce");
  const setRead = useSetRead(request);
  const rows: Experiment[] = experiments.data?.experiments ?? [];
  const channels: { id: string }[] = automation.data?.channels ?? [];
  const config = commerce.data?.data as Config | undefined;
  const currencies: string[] = config?.currencies?.enabled ?? [];
  const [draft, setDraft] = useState<Design>({
    title: "",
    locale,
    channel: "",
    currency: "",
    control: "discovery",
    treatment: "comparison",
    durationHours: 168,
    settlementDays: 30,
    minimumPerArm: 500,
    outcomeCapMinor: 10000,
    cupedTheta: 0,
  });
  const [busy, setBusy] = useState(false),
    [actionError, setError] = useState(""),
    [reports, setReports] = useState<Record<string, Readout>>({});
  const canWrite = access.includes("settings.write");
  const error =
    actionError ||
    [experiments, automation, commerce]
      .map((read) => (read.error as Error | undefined)?.message)
      .find(Boolean) ||
    (config && !config.currencies ? k("experimentUnavailable") : "");
  const refresh = async () =>
    setRead(
      "/api/intelligence/experiments",
      await request("/api/intelligence/experiments", {}, "POST"),
      {},
    );
  // The first channel and the default currency preselect the draft once known.
  const firstChannel = automation.data?.channels[0]?.id;
  const defaultCurrency = config?.currencies?.defaultCurrency;
  useEffect(() => {
    if (firstChannel === undefined || defaultCurrency === undefined) return;
    setDraft((d) => ({
      ...d,
      channel: firstChannel ?? "",
      currency: defaultCurrency,
    }));
  }, [firstChannel, defaultCurrency]);
  const run = async (operation: () => Promise<void>) => {
    if (busy) return;
    setBusy(true);
    setError("");
    try {
      await operation();
    } catch (e) {
      setError((e as Error).message);
    } finally {
      setBusy(false);
    }
  };
  const transition = (row: Experiment, operation: string) =>
    run(async () => {
      await request(
        `/api/intelligence/experiment.${operation}`,
        { id: row.id, revision: row.revision, approve: true },
        "POST",
      );
      await refresh();
    });
  const stateWord = {
    draft: "experimentDraft",
    running: "experimentRunning",
    stopped: "experimentStopped",
    completed: "experimentCompleted",
  } as const;
  const fields = [
    ["durationHours", "experimentHours", 1, 720],
    ["settlementDays", "experimentDelay", 14, 90],
    ["minimumPerArm", "experimentMinimum", 100, 5000],
    ["outcomeCapMinor", "experimentCap", 1, 1000000000000],
    ["cupedTheta", "experimentTheta", 0, 1],
  ] as const;
  return (
    <section className="knowledge-panel experiment-studio">
      <h2>{k("experiments")}</h2>
      <p>{k("experimentIntro")}</p>
      <p className="muted">{k("experimentBoundary")}</p>
      {error && <Notice tone="danger">{error}</Notice>}
      <ExperimentSuggestion
        request={request}
        disabled={busy || !canWrite}
        onUse={(design) => setDraft((d) => ({ ...d, ...design }) as Design)}
      />
      <form
        onSubmit={(e) => {
          e.preventDefault();
          void run(async () => {
            await request(
              "/api/intelligence/experiment.create",
              {
                design: {
                  ...draft,
                  locale,
                  treatment:
                    draft.control === "discovery" ? "comparison" : "discovery",
                },
                approve: true,
              },
              "POST",
            );
            await refresh();
          });
        }}
      >
        <fieldset disabled={busy || !canWrite} className="studio-form-grid">
          <Field label={k("experimentTitle")}>
            <TextInput
              required
              maxLength={240}
              value={draft.title}
              onChange={(e) => setDraft({ ...draft, title: e.target.value })}
            />
          </Field>
          <Field label={k("experimentChannel")}>
            <Select
              required
              value={draft.channel}
              onChange={(e) => setDraft({ ...draft, channel: e.target.value })}
            >
              {channels.map((c) => (
                <option key={c.id}>{c.id}</option>
              ))}
            </Select>
          </Field>
          <Field label={k("experimentCurrency")}>
            <Select
              required
              value={draft.currency}
              onChange={(e) => setDraft({ ...draft, currency: e.target.value })}
            >
              {currencies.map((c) => (
                <option key={c}>{c}</option>
              ))}
            </Select>
          </Field>
          <Field label={k("experimentControl")}>
            <Select
              value={draft.control}
              onChange={(e) => setDraft({ ...draft, control: e.target.value })}
            >
              <option value="discovery">{k("experimentDiscovery")}</option>
              <option value="comparison">{k("experimentComparison")}</option>
            </Select>
          </Field>
          {fields.map(([field, word, min, max]) => (
            <label key={field}>
              {k(word)}
              <input
                required
                type="number"
                min={min}
                max={max}
                step={field === "cupedTheta" ? "0.01" : "1"}
                value={draft[field]}
                onChange={(e) =>
                  setDraft({ ...draft, [field]: Number(e.target.value) })
                }
              />
            </label>
          ))}
          <AutomationFields
            value={draft.automation}
            onChange={(automation) => setDraft({ ...draft, automation })}
          />
          <Button
            tone="primary"
            type="submit"
            disabled={!draft.channel || !draft.currency}
          >
            {k("registerExperiment")}
          </Button>
        </fieldset>
      </form>
      <Button type="submit" disabled={busy} onClick={() => void run(refresh)}>
        {k("experimentRecheck")}
      </Button>
      {!rows.length && <p>{k("experimentEmpty")}</p>}
      {rows.map((row) => (
        <article className="knowledge-source-card" key={row.id}>
          <header>
            <h3>{row.design.title}</h3>
            <span>
              {k(
                stateWord[row.state as keyof typeof stateWord] ??
                  "experimentDraft",
              )}
            </span>
          </header>
          <p>
            {row.design.channel} · {row.design.currency} ·{" "}
            {k("experimentDuration")
              .replace("{hours}", String(row.design.durationHours))
              .replace("{days}", String(row.design.settlementDays))}
          </p>
          {row.endsAt && (
            <p>
              {k("experimentEnds")}: {date(row.endsAt)}
            </p>
          )}
          <AutomationNote
            automation={row.design.automation}
            decision={reports[row.id]?.decision}
          />
          <div className="studio-actions">
            {row.state === "draft" && (
              <button
                disabled={busy || !canWrite}
                onClick={() => void transition(row, "start")}
              >
                {k("experimentStart")}
              </button>
            )}
            {row.state === "running" && (
              <>
                <button
                  disabled={
                    busy ||
                    !canWrite ||
                    !row.endsAt ||
                    Date.parse(row.endsAt) > Date.now()
                  }
                  onClick={() => void transition(row, "finish")}
                >
                  {k("experimentFinish")}
                </button>
                <button
                  className="studio-danger"
                  disabled={busy || !canWrite}
                  onClick={() => void transition(row, "stop")}
                >
                  {k("experimentStop")}
                </button>
              </>
            )}
            <button
              disabled={busy}
              onClick={() =>
                void run(async () => {
                  const result = await request(
                    "/api/intelligence/experiment.report",
                    { id: row.id },
                    "POST",
                  );
                  setReports((previous) => ({ ...previous, [row.id]: result }));
                  // A self-running experiment may have changed state since the list was read.
                  await refresh();
                })
              }
            >
              {k("experimentReadout")}
            </button>
          </div>
          {reports[row.id] && (
            <div role="status">
              <strong>
                {k(
                  reports[row.id].causalUpliftProven
                    ? "experimentPositive"
                    : "experimentPending",
                )}
              </strong>
              <p>
                {k("experimentUnits")}:{" "}
                {reports[row.id].unitsPerArm.join(" / ")}
              </p>
              <p>
                {k("experimentNet")}: {reports[row.id].netCollectedMinor}{" "}
                {reports[row.id].currency}
              </p>
            </div>
          )}
        </article>
      ))}
    </section>
  );
}
