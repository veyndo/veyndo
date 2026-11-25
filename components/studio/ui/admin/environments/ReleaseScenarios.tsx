/** Actor-private saved scenario evidence sits beside native release diffs without becoming an approval gate. */
import { useRead } from "../../../../../frontend/src/admin/shell/useRead";
import {
  Button,
  Field,
  Notice,
  Select,
} from "../../../../../frontend/src/kit/ui";
import { useMemo, useState } from "react";
import type { RequestFn } from "../../../../../frontend/src/admin/shell/studio-types";
import type {
  AttackSummary,
  Report,
  Scenario,
} from "../../../../intelligence/ui/admin/scenarios/scenario-types";
import { canTestCustomers } from "../../../../intelligence/ui/admin/scenarios/scenario-access";
import ReleaseShopPreview from "./ReleaseShopPreview";
import ReleaseAttack from "./ReleaseAttack";
import ScenarioReport from "../../../../intelligence/ui/admin/scenarios/ScenarioReport";
import { useLocale } from "../../../../../frontend/src/shared/i18n/i18n";
import { scenarioLabel } from "../../../../intelligence/ui/admin/scenarios/scenario-history";
import { useReleaseTestText } from "./release-tests-i18n";
import "../../../../intelligence/ui/admin/scenarios/scenarios.css";
import "./release-tests.css";
export default function ReleaseScenarios({
  environment,
  scope,
  rights,
  sandboxRequest,
  onOpen,
}: {
  environment: string;
  scope?: object;
  rights: string[];
  sandboxRequest: (environment: string) => RequestFn;
  onOpen: (
    environment: string,
    channel: string,
    scenario?: string,
  ) => void | Promise<void>;
}) {
  const request = useMemo<RequestFn>(() => {
    const scoped = sandboxRequest(environment);
    return (...args) => scoped(...args);
  }, [sandboxRequest, environment, scope]);
  const allowed = !!environment && canTestCustomers(rights);
  return allowed ? (
    <Review request={request} environment={environment} onOpen={onOpen} />
  ) : null;
}
function Review({
  request,
  environment,
  onOpen,
}: {
  request: RequestFn;
  environment: string;
  onOpen: (
    environment: string,
    channel: string,
    scenario?: string,
  ) => void | Promise<void>;
}) {
  const t = useReleaseTestText();
  const { locale } = useLocale();
  const [refresh, setRefresh] = useState(0);
  const [navigation, setNavigation] = useState<{
    request: RequestFn;
    busy: boolean;
    error?: boolean;
  }>();
  const [selection, setSelection] = useState<{
    request: RequestFn;
    id: string;
  }>();
  const scenarios = useRead(request, "/api/agent/scenarios");
  const currentList:
    | { items: Scenario[]; attack?: AttackSummary | null; error?: boolean }
    | undefined =
    scenarios.status === "error"
      ? { items: [], error: true }
      : scenarios.data && !scenarios.fetching
        ? { items: scenarios.data.scenarios, attack: scenarios.data.attack }
        : undefined;
  // A selection survives a reload only while its scenario is still listed.
  const id =
    selection?.request === request &&
    currentList?.items.some((item) => item.id === selection.id)
      ? selection.id
      : "";
  const checked = useRead<Report>(
    request,
    id ? `/api/agent/scenarios/${encodeURIComponent(id)}` : null,
  );
  const currentResult:
    { report?: Report; error?: boolean; at: string } | undefined =
    !id || checked.fetching
      ? undefined
      : checked.status === "error"
        ? { error: true, at: "" }
        : checked.data
          ? {
              report: checked.data,
              at: new Date(
                checked.updatedAt ?? Date.now(),
              ).toLocaleTimeString(),
            }
          : undefined;
  const report = currentResult?.report;
  const loading = !currentList || (!!id && !currentResult);
  const navigate = async () => {
    setNavigation({ request, busy: true });
    try {
      await onOpen(
        environment,
        report?.runs[0]?.scenario?.channel ?? "default",
        report?.id,
      );
      setNavigation({ request, busy: false });
    } catch {
      setNavigation({ request, busy: false, error: true });
    }
  };
  return (
    <section className="release-tests" aria-label={t("title")}>
      <h2>{t("title")}</h2>
      <p className="release-tests-hint">{t("hint")}</p>
      <div className="release-tests-controls">
        <Field label={t("select")}>
          <Select
            value={id}
            disabled={!currentList?.items.length}
            onChange={(e) => setSelection({ request, id: e.target.value })}
          >
            <option value="">{t("choose")}</option>
            {currentList?.items.map((item) => (
              <option key={item.id} value={item.id}>
                {scenarioLabel(item, locale)}
              </option>
            ))}
          </Select>
        </Field>
        <Button
          type="button"
          disabled={loading}
          onClick={() => {
            setRefresh((v) => v + 1);
            void scenarios.refetch();
            if (id) void checked.refetch();
          }}
        >
          {t("refresh")}
        </Button>
        <Button
          type="button"
          disabled={navigation?.request === request && navigation.busy}
          onClick={() => void navigate()}
        >
          {t(report ? "open" : "new")}
        </Button>
      </div>
      {navigation?.request === request && navigation.error && (
        <Notice tone="danger">{t("openError")}</Notice>
      )}
      {loading && <p role="status">{t("loading")}</p>}
      {(currentList?.error || currentResult?.error) && (
        <Notice tone="danger">{t("error")}</Notice>
      )}
      {currentList && !currentList.error && !currentList.items.length && (
        <p>{t("empty")}</p>
      )}
      {currentList && !currentList.error && (
        <ReleaseAttack attack={currentList.attack} />
      )}
      {report && (
        <>
          <p className="release-tests-time">
            {t("checked")}: <time>{currentResult?.at}</time>
          </p>
          <ScenarioReport report={report} />
          <ReleaseShopPreview
            key={`${report.id}:${refresh}`}
            environment={environment}
            request={request}
            report={report}
          />
        </>
      )}
    </section>
  );
}
