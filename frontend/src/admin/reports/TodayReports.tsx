/** Shop-today presentation is saved in the existing actor-private revisioned workspace, never in shop configuration. */
import { useRead, useSetRead } from "../shell/useRead";
import { Button } from "../../kit/ui";
import { useEffect, useRef, useState } from "react";
import { useStudio } from "../shell/StudioContext";
import type { WorkspaceRecord } from "../workspace/workspace-model";
import { useReportText } from "./report-i18n";
import { defaultReports, type ReportLayout } from "./model";
import ReportBoard from "./ReportBoard";
export default function TodayReports() {
  const { request, liveRequest, workspace, access } = useStudio(),
    t = useReportText();
  const [layout, setLayout] = useState<ReportLayout>(defaultReports),
    [saveError, setError] = useState(false),
    [busy, setBusy] = useState(false),
    [saved, setSaved] = useState(false),
    [attempt, setAttempt] = useState(0);
  const scope = useRef(0);
  const recordRead = useRead<WorkspaceRecord>(
    liveRequest,
    "/api/studio/workspace",
    {
      version: attempt,
    },
  );
  const setRead = useSetRead(liveRequest);
  const current = recordRead.data;
  const error = saveError || recordRead.status === "error";
  useEffect(() => {
    const generation = ++scope.current;
    setError(false);
    setBusy(false);
    setSaved(false);
    return () => {
      if (scope.current === generation) scope.current++;
    };
  }, [workspace, liveRequest, attempt]);
  // The stored layout replaces the board whenever the record is (re)loaded.
  const loadedLayout = recordRead.data?.data.todayReports;
  useEffect(() => {
    if (recordRead.data) setLayout(loadedLayout ?? defaultReports());
  }, [recordRead.data]);
  const save = async () => {
    if (!current || busy) return;
    const generation = scope.current;
    setBusy(true);
    setError(false);
    setSaved(false);
    try {
      const next = await liveRequest(
        "/api/studio/workspace",
        {
          revision: current.revision,
          data: { ...current.data, todayReports: layout },
        },
        "PUT",
      );
      if (generation === scope.current) {
        setRead("/api/studio/workspace", next, undefined);
        setSaved(true);
      }
    } catch {
      if (generation === scope.current) setError(true);
    } finally {
      if (generation === scope.current) setBusy(false);
    }
  };
  if (error && !current)
    return (
      <div className="report-error" role="alert">
        <p>{t("error")}</p>
        <Button type="submit" onClick={() => setAttempt((v) => v + 1)}>
          {t("reload")}
        </Button>
      </div>
    );
  if (!current) return <p role="status">{t("loading")}</p>;
  return (
    <>
      {error && (
        <div role="alert" className="report-error">
          <p>{t("saveError")}</p>
          <Button type="submit" onClick={() => void save()}>
            {t("save")}
          </Button>
          <Button type="submit" onClick={() => setAttempt((v) => v + 1)}>
            {t("reload")}
          </Button>
        </div>
      )}
      <ReportBoard
        layout={layout}
        onChange={(v) => {
          setLayout(v);
          setSaved(false);
        }}
        request={request}
        rights={access}
        save={() => void save()}
        saving={busy}
        saved={saved}
      />
    </>
  );
}
