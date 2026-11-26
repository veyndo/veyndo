/** Long actions use the same app/outbox contract, with progress and explicit uncertain-outcome review. */
import { Button, Notice } from "../../../../../frontend/src/kit/ui";
import { useState } from "react";
import type { RequestFn } from "../../../../../frontend/src/admin/shell/studio-types";
import { useRead } from "../../../../../frontend/src/admin/shell/useRead";
import AppJobArtifact from "./AppJobArtifact";
import ConfirmDialog from "../../../../../frontend/src/shared/ui/ConfirmDialog";
import { contentText } from "../../../../../frontend/src/shared/i18n/content-language";
import { useContentLanguage } from "../../../../../frontend/src/shared/i18n/ContentLanguage";
import { useLocale } from "../../../../../frontend/src/shared/i18n/i18n";
import { useAppOperationsText } from "../../../../../frontend/src/shared/i18n/app-operations-i18n";
type Job = {
  id: string;
  action: string;
  status:
    | "queued"
    | "running"
    | "succeeded"
    | "failed"
    | "uncertain"
    | "cancel_requested"
    | "cancelled";
  progress: number;
  message: Record<string, string>;
  revision: number;
  result: unknown;
};
export default function AppJobs({
  app,
  request,
}: {
  app: string;
  request: RequestFn;
}) {
  const t = useAppOperationsText(),
    { mainLocale } = useContentLanguage(),
    { locale } = useLocale();
  const [actionError, setError] = useState(""),
    [busy, setBusy] = useState(false),
    [selected, setSelected] = useState<{ job: Job; operation: string } | null>(
      null,
    );
  // Polls every ten seconds while the page is visible.
  const jobsRead = useRead(request, `/api/apps/${app}/jobs`, {
    refetchMs: 10000,
  });
  const jobs: Job[] = jobsRead.data?.jobs ?? [];
  const error =
    actionError || (jobsRead.error as Error | undefined)?.message || "";
  const refresh = async () => {
    setError("");
    await jobsRead.refetch();
  };
  return (
    <section className="app-contract-summary">
      <header className="app-detail-heading">
        <h3>{t("jobs")}</h3>
        <Button type="submit" disabled={busy} onClick={() => void refresh()}>
          {t("refresh")}
        </Button>
      </header>
      <p>{t("jobHint")}</p>
      {jobs.map((j) => (
        <article key={j.id} className="app-job">
          <strong>{j.action}</strong> · {t(j.status)}
          <progress max={100} value={j.progress} />
          <p>{contentText(j.message, locale, mainLocale)}</p>
          <div className="app-actions">
            {(["queued", "running"].includes(j.status)
              ? ["cancel"]
              : ["failed", "uncertain"].includes(j.status)
                ? [
                    "retry",
                    ...(j.status === "uncertain"
                      ? ["resolve_cancelled"]
                      : ["archive"]),
                  ]
                : ["succeeded", "cancelled"].includes(j.status)
                  ? ["archive"]
                  : []
            ).map((op) => (
              <Button
                type="submit"
                key={op}
                disabled={busy}
                onClick={() => setSelected({ job: j, operation: op })}
              >
                {t(op as "cancel" | "retry" | "archive" | "resolve_cancelled")}
              </Button>
            ))}
          </div>
          {j.status === "succeeded" && (
            <AppJobArtifact app={app} result={j.result} request={request} />
          )}
          {j.result != null && (
            <details>
              <summary>{t("result")}</summary>
              <pre>{JSON.stringify(j.result, null, 2)}</pre>
            </details>
          )}
        </article>
      ))}
      {!jobs.length && <p>{t("empty")}</p>}
      {error && <Notice tone="danger">{error}</Notice>}
      {selected && (
        <ConfirmDialog
          title={t("jobs")}
          confirmLabel={t(selected.operation as "cancel")}
          disabled={busy}
          onCancel={() => setSelected(null)}
          onConfirm={() => {
            setBusy(true);
            void request(`/api/apps/${app}/jobs/${selected.job.id}`, {
              operation: selected.operation,
              revision: selected.job.revision,
              approveUnknownOutcome: true,
            })
              .then(() => {
                setSelected(null);
                return refresh();
              })
              .catch((e) => setError(e.message))
              .finally(() => setBusy(false));
          }}
        >
          <p>
            {t(
              selected.operation === "retry" ||
                selected.operation === "resolve_cancelled"
                ? "jobUnknown"
                : "jobConfirm",
            )}
          </p>
        </ConfirmDialog>
      )}
    </section>
  );
}
