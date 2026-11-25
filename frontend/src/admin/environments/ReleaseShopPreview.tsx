/** Explicit native sandbox preview rechecks the saved run; it never attests a historical render. */
import { Button, Field, LinkButton, Notice, Select } from "../../kit/ui";
import { useEffect, useRef, useState } from "react";
import type { RequestFn } from "../shell/studio-types";
import type { Report, Run } from "../scenarios/scenario-types";
import ScenarioEvidence from "../scenarios/ScenarioEvidence";
import { useReleasePreviewText } from "./release-preview-i18n";
import "./release-preview.css";
export default function ReleaseShopPreview({
  environment,
  request,
  report,
}: {
  environment: string;
  request: RequestFn;
  report: Report;
}) {
  const t = useReleasePreviewText();
  const [task, setTask] = useState(report.runs[0]?.taskId ?? "");
  const [device, setDevice] = useState("desktop");
  const [view, setView] = useState<{ run: Run; generation: number }>();
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState(false);
  const generation = useRef(0);
  useEffect(
    () => () => {
      generation.current++;
    },
    [],
  );
  const close = () => {
    generation.current++;
    setView(undefined);
    setBusy(false);
    setError(false);
  };
  const open = async () => {
    const ticket = ++generation.current;
    setBusy(true);
    setError(false);
    setView(undefined);
    try {
      const latest: Report = await request(
        `/api/agent/scenarios/${encodeURIComponent(report.id)}`,
      );
      const run = latest.runs.find((r) => r.taskId === task);
      if (
        latest.id !== report.id ||
        !run?.scenario?.channel ||
        !run.scenario.profile.locale
      )
        throw new Error();
      if (ticket === generation.current) setView({ run, generation: ticket });
    } catch {
      if (ticket === generation.current) setError(true);
    } finally {
      if (ticket === generation.current) setBusy(false);
    }
  };
  const scenario = view?.run.scenario;
  const url = scenario
    ? `/?${new URLSearchParams({
        shop: environment,
        sandbox: "1",
        channel: scenario.channel,
        lang: scenario.profile.locale,
        language: scenario.profile.locale,
      })}`
    : undefined;
  return (
    <section className="release-shop-preview" aria-label={t("title")}>
      <h3>{t("title")}</h3>
      <p>{t("hint")}</p>
      <div className="release-preview-controls">
        {report.runs.length > 1 && (
          <Field label={t("profile")}>
            <Select
              value={task}
              onChange={(e) => {
                close();
                setTask(e.target.value);
              }}
            >
              {report.runs.map((run) => (
                <option key={run.taskId} value={run.taskId}>
                  {run.scenario?.profile.name ?? run.taskId}
                </option>
              ))}
            </Select>
          </Field>
        )}
        <Button
          type="button"
          disabled={busy || !task}
          onClick={() => void open()}
        >
          {t(view ? "reload" : "open")}
        </Button>
        {(view || busy) && (
          <Button type="button" onClick={close}>
            {t("close")}
          </Button>
        )}
      </div>
      {busy && <p role="status">{t("loading")}</p>}
      {error && <Notice tone="danger">{t("error")}</Notice>}
      {view && scenario && url && (
        <>
          <ScenarioEvidence run={view.run} />
          <div className="release-preview-controls">
            <p className="release-preview-context">
              {scenario.channel} · {scenario.profile.locale}
            </p>
            <Field label={t("device")}>
              <Select
                value={device}
                onChange={(e) => setDevice(e.target.value)}
              >
                <option value="desktop">{t("desktop")}</option>
                <option value="mobile">{t("mobile")}</option>
              </Select>
            </Field>
            <LinkButton href={url} target="_blank" rel="noopener noreferrer">
              {t("separate")}
            </LinkButton>
          </div>
          <div className="release-preview-canvas" data-device={device}>
            <iframe
              key={view.generation}
              title={t("title")}
              src={url}
              sandbox="allow-scripts allow-same-origin allow-forms"
            />
          </div>
          <p className="release-preview-help">{t("help")}</p>
        </>
      )}
    </section>
  );
}
