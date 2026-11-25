/** AI generation is an explicit reviewed proposal, with conflict protection and no automatic canvas replacement. */
import { Button, Notice } from "../../../../../frontend/src/kit/ui";
import { useCanvasText } from "../../../../../frontend/src/shared/canvas/canvas-i18n";
import type { AppStudio } from "./useAppStudio";
import { proposalChanges } from "./app-proposal";
import { useBuildingText } from "../../../../../frontend/src/shared/i18n/app-building-i18n";
import { contentText } from "../../../../../frontend/src/shared/i18n/content-language";
import { useLocale } from "../../../../../frontend/src/shared/i18n/i18n";
import type { NativeLifecycle } from "../../../../../frontend/src/shared/apps/native/types";
import { useLifecycleText } from "../../../../../frontend/src/shared/i18n/native-lifecycle-i18n";
import { useWorkflowStateText } from "../../../../../frontend/src/shared/i18n/workflow-state-text";
import { usePermissionText } from "../../../../identity/ui/admin/team/permission-i18n";
export default function AppProposalReview({
  studio,
  comparing,
  onPreview,
}: {
  studio: AppStudio;
  comparing?: boolean;
  onPreview?: (value: boolean) => void;
}) {
  const cv = useCanvasText();
  const t = useBuildingText(),
    life = useLifecycleText(),
    { locale } = useLocale(),
    p = studio.proposal;
  if (!p) return null;
  const stale =
      p.base !== JSON.stringify(studio.manifest) ||
      p.build.environment !== studio.env,
    changes = proposalChanges(studio.manifest, p.build.manifest);
  return (
    <section className="app-proposal" aria-label={t("proposal")}>
      <h3>{t("proposal")}</h3>
      <p>{t("proposalHint")}</p>
      <p>{contentText(p.build.summary, locale, studio.mainLocale)}</p>
      {p.build.designEvidence && (
        <details>
          <summary>{cv("references")}</summary>
          <ul>
            {p.build.designEvidence.sources.map((source, index) => (
              <li key={index}>
                <code>{source.url ?? cv("references")}</code> ·{" "}
                {cv(
                  source.status === "provided" ||
                    source.status === "partial" ||
                    source.status === "unavailable"
                    ? source.status
                    : "references",
                )}
                {source.digest && (
                  <small> · {source.digest.slice(0, 12)}</small>
                )}
                {source.status !== "provided" && (
                  <p>
                    {cv(
                      source.status === "partial"
                        ? "captureHint"
                        : "unavailableHint",
                    )}
                  </p>
                )}
              </li>
            ))}
          </ul>
        </details>
      )}
      <div className="app-proposal-groups">
        {(["models", "surfaces", "workflows", "access"] as const).map(
          (group) => (
            <div key={group}>
              <strong>
                {group === "workflows" ? life("title") : t(group)}
              </strong>
              <ul>
                {changes
                  .filter((c) => c.group === group)
                  .map((c) => (
                    <li key={c.id}>
                      <span>{t(c.kind)}</span>{" "}
                      <code>
                        {group === "workflows"
                          ? life(c.id as "order" | "quote")
                          : c.id}
                      </code>
                      {group === "workflows" &&
                        p.build.manifest.lifecycles
                          ?.filter((w) => w.object === c.id)
                          .map((w) => (
                            <WorkflowSummary
                              key={w.object}
                              contract={w}
                              mainLocale={studio.mainLocale}
                            />
                          ))}
                    </li>
                  ))}
              </ul>
              {!changes.some((c) => c.group === group) && (
                <small>{t("noChanges")}</small>
              )}
            </div>
          ),
        )}
      </div>
      {stale && <Notice tone="danger">{t("conflict")}</Notice>}
      <div className="app-agent-options">
        {onPreview && (
          <Button
            type="button"
            aria-pressed={!!comparing}
            disabled={stale}
            onClick={() => onPreview(!comparing)}
          >
            {cv(comparing ? "before" : "after")}
          </Button>
        )}
        <Button
          tone="primary"
          type="button"
          disabled={stale || studio.busy}
          onClick={studio.acceptProposal}
        >
          {t("accept")}
        </Button>
        <Button type="button" onClick={studio.discardProposal}>
          {t("discard")}
        </Button>
      </div>
    </section>
  );
}
function WorkflowSummary({
  contract,
  mainLocale,
}: {
  contract: NativeLifecycle;
  mainLocale: string;
}) {
  const life = useLifecycleText(),
    nativeState = useWorkflowStateText(),
    permission = usePermissionText(),
    { locale } = useLocale();
  const name = (id: string) =>
    contentText(
      contract.states.find((s) => s.id === id)?.label ?? {},
      locale,
      mainLocale,
    ) || nativeState(contract.object, id);
  return (
    <div className="app-workflow-edges">
      {contract.transitions.map((edge) => (
        <span key={edge.id}>
          <strong>{contentText(edge.label, locale, mainLocale)}</strong>
          {name(edge.from)} → {name(edge.to)}
          {edge.guards?.map((g) => (
            <small key={g}>{life(g as "payment.ready")}</small>
          ))}
          {edge.requiredPermissions?.map((scope) => (
            <small key={scope}>
              {permission(scope)} ({scope})
            </small>
          ))}
        </span>
      ))}
    </div>
  );
}
