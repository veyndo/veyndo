/** A compact private customer lab on the existing Studio scope and durable run controls. */
import type { ApiPath } from "../../../../../frontend/src/kit/api";
import {
  useRead,
  useSetRead,
} from "../../../../../frontend/src/admin/shell/useRead";
import { Button } from "../../../../../frontend/src/kit/ui";
import { useEffect, useMemo, useRef, useState } from "react";
import { rememberScenario } from "./scenario-history";
import { useStudio } from "../../../../../frontend/src/admin/shell/StudioContext";
import type { RequestFn } from "../../../../../frontend/src/admin/shell/studio-types";
import Icon from "../../../../../frontend/src/shared/ui/Icon";
import { useScenarioText } from "./scenario-i18n";
import { canTestCustomers } from "./scenario-access";
import ScenarioHistory from "./ScenarioHistory";
import ScenarioProfiles from "./ScenarioProfiles";
import ScenarioSuggestions from "./ScenarioSuggestions";
import ScenarioReport from "./ScenarioReport";
import ScenarioComparison from "./ScenarioComparison";
import ScenarioSource from "./ScenarioSource";
import ScenarioAttacks from "./ScenarioAttacks";
import ScenarioTwin from "./ScenarioTwin";
import type {
  Profile,
  Configuration,
  Scenario,
  Report,
  Run,
} from "./scenario-types";
import "./scenarios.css";
export default function ScenarioStudio({
  request: baseRequest,
  channel = "default",
  sourceTaskId,
  initialScenarioId,
}: {
  request: RequestFn;
  channel?: string;
  sourceTaskId?: string;
  initialScenarioId?: string;
}) {
  const request = useMemo<RequestFn>(
    () => (path, body, method, options) =>
      baseRequest(path, body, method, { ...options, salesChannel: channel }),
    [baseRequest, channel],
  );
  const { environment, access, selectTab } = useStudio(),
    t = useScenarioText();
  const allowed = canTestCustomers(access);
  const [instruction, setInstruction] = useState(""),
    [profiles, setProfiles] = useState<Profile[]>([]);
  const [selected, setSelected] = useState(""),
    [baseline, setBaseline] = useState("");
  const [actionError, setError] = useState(""),
    [busy, setBusy] = useState(false);
  const [notice, setNotice] = useState("");
  const [source, setSource] = useState(sourceTaskId);
  const [refresh, setRefresh] = useState(0);
  const [inference, setInference] = useState<
    { provider: string; model?: string } | undefined
  >();
  const generation = useRef(0),
    retry = useRef<{ digest: string; id: string } | undefined>(undefined);
  const currentSelection = useRef(selected);
  currentSelection.current = selected;
  const open = !!environment && allowed;
  const scenarioPath = (id: string): ApiPath =>
    `/api/agent/scenarios/${encodeURIComponent(id)}`;
  const listRead = useRead(request, open ? "/api/agent/scenarios" : null);
  const setRead = useSetRead(request);
  const config: Configuration | undefined = listRead.data?.configuration;
  const scenarios: Scenario[] = listRead.data?.scenarios ?? [];
  // Running scenarios poll every 1.8 seconds until they settle.
  const running = (value?: Report) =>
    !!value?.runs.some((r) => ["queued", "running"].includes(r.state));
  const [reportPolling, setReportPolling] = useState(false);
  const reportRead = useRead<Report>(
    request,
    open && selected ? scenarioPath(selected) : null,
    { refetchMs: reportPolling ? 1800 : undefined },
  );
  const report = reportRead.data;
  // Saved changes elsewhere ask for the selected report again.
  useEffect(() => {
    if (refresh) void reportRead.refetch();
  }, [refresh]);
  const [comparisonPolling, setComparisonPolling] = useState(false);
  const comparisonRead = useRead<Report>(
    request,
    open && baseline ? scenarioPath(baseline) : null,
    { refetchMs: comparisonPolling ? 1800 : undefined },
  );
  const comparison = comparisonRead.data;
  useEffect(() => setReportPolling(running(report)), [report]);
  useEffect(() => setComparisonPolling(running(comparison)), [comparison]);
  const error =
    actionError ||
    [listRead, reportRead, comparisonRead]
      .map((read) => (read.error as Error | undefined)?.message)
      .find(Boolean) ||
    "";
  useEffect(() => {
    ++generation.current;
    setSelected(initialScenarioId ?? "");
    setBaseline("");
    setProfiles([]);
    setBusy(false);
    setError("");
    setNotice("");
    setInference(undefined);
    setInstruction("");
    setSource(sourceTaskId);
    retry.current = undefined;
    return () => {
      generation.current++;
    };
  }, [request, environment, allowed, sourceTaskId]);
  // A loaded configuration proposes the three default customer profiles.
  useEffect(() => {
    if (!config) return;
    const c = config;
    setProfiles(
      (["budget", "first", "fast"] as const).map((id) => ({
        id,
        name: t(`${id}Name`),
        goal: t(`${id}Goal`),
        budgetMinor: 300 * 10 ** c.scale,
        locale: c.locales.includes(c.mainLocale)
          ? c.mainLocale
          : (c.locales[0] ?? ""),
        country: c.countries[0] ?? "",
      })),
    );
  }, [config]);
  useEffect(() => {
    if (initialScenarioId !== currentSelection.current)
      setSelected(initialScenarioId ?? "");
  }, [initialScenarioId]);
  const start = async () => {
    if (busy || !config) return;
    setBusy(true);
    setError("");
    const scope = generation.current;
    const body = {
      title: instruction.trim().slice(0, 80),
      instruction: instruction.trim(),
      profiles,
      ...(source ? { sourceTaskId: source } : {}),
      ...(inference ? { inference } : {}),
    };
    const digest = JSON.stringify(body);
    if (retry.current?.digest !== digest)
      retry.current = { digest, id: crypto.randomUUID() };
    try {
      const value = await request(
        "/api/agent/scenarios",
        { ...body, requestId: retry.current.id },
        "POST",
      );
      if (scope !== generation.current) return;
      rememberScenario(value.id, channel);
      setSelected(value.id);
      setRead(scenarioPath(value.id), value, undefined);
      retry.current = undefined;
      const list = await request("/api/agent/scenarios");
      if (scope === generation.current) setRead("/api/agent/scenarios", list);
    } catch (e) {
      if (scope === generation.current) setError((e as Error).message);
    } finally {
      if (scope === generation.current) setBusy(false);
    }
  };
  const control = async (run: Run, action: "cancel" | "resume") => {
    setError("");
    const scope = generation.current;
    try {
      await request(
        `/api/agent/runs/${run.taskId}/${action}`,
        { revision: run.revision, approve: true },
        "POST",
      );
      const value = await request(`/api/agent/scenarios/${selected}`);
      if (
        scope === generation.current &&
        selected === currentSelection.current
      ) {
        setRead(scenarioPath(selected), value, undefined);
        setRefresh((value) => value + 1);
      }
    } catch (e) {
      if (scope === generation.current && selected === currentSelection.current)
        setError((e as Error).message);
    }
  };
  const repeat = () => {
    const saved = report?.runs
      .map((r) => r.scenario)
      .filter((s) => !!s)
      .sort((a, b) => a.index - b.index);
    if (!saved?.length || !config) return;
    if (
      saved[0].currency !== config.currency ||
      saved[0].channel !== config.channel
    ) {
      setError(t("contextChanged"));
      return;
    }
    setProfiles(saved.map((s) => s.profile));
    setInstruction(saved[0].instruction);
    setInference(saved[0].inference ?? undefined);
    setSource(saved[0].source?.taskId);
    setBaseline(report!.id);
    setNotice(t("repeated"));
  };
  const currentReport = report?.id === selected ? report : undefined;
  const currentComparison =
    comparison?.id === baseline && baseline !== selected
      ? comparison
      : undefined;
  return (
    <section className="scenario-studio">
      <header className="scenario-intro">
        <span className="scenario-emblem">
          <Icon name="people" size={28} />
        </span>
        <div>
          <h2>{t("title")}</h2>
          <p>{t("intro")}</p>
        </div>
      </header>
      {!allowed ? (
        <p>{t("rights")}</p>
      ) : !environment ? (
        <div className="scenario-empty">
          <p>{t("private")}</p>
          <Button
            tone="primary"
            type="submit"
            onClick={() => selectTab("environments")}
          >
            {t("sandbox")}
          </Button>
        </div>
      ) : (
        <>
          {error && (
            <p className="scenario-error" role="alert">
              {error}
            </p>
          )}
          {notice && <p role="status">{notice}</p>}
          {source && (
            <ScenarioSource
              taskId={source}
              onClear={() => setSource(undefined)}
            />
          )}
          <ScenarioTwin
            request={request}
            refresh={scenarios.length + refresh}
            access={access}
            onOpen={selectTab}
          />
          <label className="scenario-brief">
            {t("goal")}
            <textarea
              value={instruction}
              maxLength={2000}
              onChange={(e) => setInstruction(e.target.value)}
              rows={3}
            />
          </label>
          {config && (
            <>
              <ScenarioSuggestions
                config={config}
                onApply={setProfiles}
                disabled={busy}
              />
              <ScenarioProfiles
                profiles={profiles}
                config={config}
                onChange={setProfiles}
              />
            </>
          )}
          <footer className="scenario-start">
            <p>{t("boundary")}</p>
            <Button
              tone="primary"
              type="submit"
              disabled={
                busy ||
                !instruction.trim() ||
                !profiles.length ||
                profiles.some(
                  (p) => !p.name.trim() || !p.goal.trim() || p.budgetMinor < 1,
                )
              }
              onClick={() => void start()}
            >
              <Icon name="arrow" />
              {t(busy ? "starting" : "start")}
            </Button>
          </footer>
          <ScenarioAttacks
            request={request}
            onSaved={() => setRefresh((value) => value + 1)}
          />
          {!!scenarios.length && (
            <ScenarioHistory
              scenarios={scenarios}
              selected={selected}
              baseline={baseline}
              onSelect={(id) => {
                rememberScenario(id, channel);
                setSelected(id);
              }}
              onCompare={setBaseline}
            />
          )}
          {currentReport && (
            <div className="scenario-actions">
              <Button type="submit" onClick={repeat}>
                {t("repeat")}
              </Button>
              <Button type="submit" onClick={() => selectTab("productData")}>
                {t("repairProducts")}
              </Button>
              {access.includes("settings.read") && (
                <Button type="submit" onClick={() => selectTab("commerce")}>
                  {t("repairSettings")}
                </Button>
              )}
              <Button type="submit" onClick={() => selectTab("orders")}>
                {t("openOrders")}
              </Button>
            </div>
          )}
          {currentComparison && currentReport && (
            <ScenarioComparison
              before={currentComparison}
              after={currentReport}
            />
          )}
          <div className={currentComparison ? "scenario-comparison" : ""}>
            {currentComparison && (
              <div>
                <h3>{t("before")}</h3>
                <ScenarioReport report={currentComparison} />
              </div>
            )}
            <div>
              {currentComparison && <h3>{t("after")}</h3>}
              {currentReport && (
                <ScenarioReport
                  report={currentReport}
                  onControl={(r, a) => void control(r, a)}
                />
              )}
            </div>
          </div>
        </>
      )}
    </section>
  );
}
