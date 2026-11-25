/** Unified knowledge workspace connects sources, product evidence, observations and no-model retrieval previews. */
import {
  useRead,
  useSetRead,
} from "../../../../../frontend/src/admin/shell/useRead";
import { Button } from "../../../../../frontend/src/kit/ui";
import { useEffect, useState } from "react";
import {
  useKnowledgeText,
  type KnowledgeWord,
} from "../../../../../frontend/src/shared/i18n/knowledge-i18n";
import { useStudioText } from "../../../../../frontend/src/shared/i18n/studio-ui-i18n";
import { useLocale } from "../../../../../frontend/src/shared/i18n/i18n";
import Icon from "../../../../../frontend/src/shared/ui/Icon";
import type {
  Overview,
  RequestFn,
} from "../../../../../frontend/src/admin/shell/studio-types";
import type { Workspace } from "./knowledge-types";
import BehaviorView from "./behavior/BehaviorView";
import KnowledgeNavigation from "./KnowledgeNavigation";
import KnowledgeOverview from "./KnowledgeOverview";
import KnowledgeSources from "./KnowledgeSources";
import KnowledgeExplorer from "./KnowledgeExplorer";
import KnowledgePreview from "./KnowledgePreview";
import ExternalKnowledge from "./ExternalKnowledge";
import ExperimentStudio from "./ExperimentStudio";
import ScenarioStudio from "../scenarios/ScenarioStudio";
import GuardrailSettings from "./GuardrailSettings";
import SovereignAI from "./SovereignAI";
import EvidenceReview from "./EvidenceReview";
import MemoryView from "./MemoryView";
import "./knowledge.css";
import "./knowledge-sources.css";
import "./knowledge-evidence.css";
export function KnowledgeView({
  focus,
  data,
  request,
  onIntent,
  onProduct,
  onEntity,
}: {
  focus: string;
  data: Overview;
  request: RequestFn;
  onIntent: (s: string) => void;
  onProduct: (id: string) => void;
  onEntity?: (kind: "customers" | "orders", id: string) => void;
}) {
  const k = useKnowledgeText(),
    { number } = useLocale();
  const { u } = useStudioText();
  const [dirty, setDirty] = useState(false),
    [pending, setPending] = useState<string>();
  const selectKnowledge = (next: string) => {
    if (next === tab) return;
    if (dirty) setPending(next);
    else setTab(next);
  };
  const [tab, setTab] = useState(() =>
      new URLSearchParams(location.search).get("knowledgeSection") ===
      "scenarios"
        ? "scenarios"
        : "overview",
    ),
    [actionError, setError] = useState(""),
    [working, setBusy] = useState(false),
    [create, setCreate] = useState(false);
  const workspaceRead = useRead<Workspace>(request, "/api/knowledge/workspace");
  const setRead = useSetRead(request);
  const workspace = workspaceRead.data;
  const busy = working || workspaceRead.status === "loading";
  const error =
    actionError || (workspaceRead.error as Error | undefined)?.message || "";
  useEffect(() => {
    const url = new URL(location.href);
    if (tab === "scenarios") url.searchParams.set("knowledgeSection", tab);
    else url.searchParams.delete("knowledgeSection");
    history.replaceState(null, "", url);
  }, [tab]);
  const load = async () => {
    setBusy(true);
    setError("");
    try {
      setRead(
        "/api/knowledge/workspace",
        await request("/api/knowledge/workspace"),
      );
    } catch (e) {
      setError((e as Error).message);
    } finally {
      setBusy(false);
    }
  };
  const newSource = () => {
    setTab("sources");
    setCreate(true);
  };
  return (
    <div className="studio-page knowledge-workspace">
      <div className="page-intro knowledge-intro">
        <div>
          <span className="kicker">{k(tab as KnowledgeWord)}</span>
          <h1>{k("heading")}</h1>
          <p>{k("intro")}</p>
        </div>
        <Button type="submit" disabled={busy} onClick={() => void load()}>
          <Icon name="refresh" />
          {k("refresh")}
        </Button>
      </div>
      {pending && (
        <div className="knowledge-alert" role="alert">
          <p>{u("discardHint")}</p>
          <Button type="submit" onClick={() => setPending(undefined)}>
            {u("keepEditing")}
          </Button>
          <Button
            type="submit"
            onClick={() => {
              setDirty(false);
              setTab(pending);
              setPending(undefined);
            }}
          >
            {u("discard")}
          </Button>
        </div>
      )}
      <div className="knowledge-workbench">
        <KnowledgeNavigation tab={tab} onTab={selectKnowledge} />
        <div className="knowledge-workbench-content">
          {error && (
            <div className="knowledge-alert" role="alert">
              {error}
              <Button type="submit" disabled={busy} onClick={() => void load()}>
                {k("retry")}
              </Button>
            </div>
          )}
          {busy && !workspace && <p role="status">{k("loading")}</p>}
          {workspace && (
            <>
              {tab === "overview" && (
                <KnowledgeOverview
                  workspace={workspace}
                  onTab={setTab}
                  onCreate={newSource}
                />
              )}{" "}
              {tab === "sources" && (
                <>
                  <KnowledgeSources
                    workspace={workspace}
                    request={request}
                    onChange={() => void load()}
                    create={create}
                    onCreateDone={() => setCreate(false)}
                  />
                  <ExternalKnowledge request={request} />
                </>
              )}
              {tab === "explorer" && (
                <KnowledgeExplorer
                  focus={focus}
                  products={data.products}
                  request={request}
                  onProduct={onProduct}
                  onIntent={onIntent}
                />
              )}
              {tab === "preview" && (
                <KnowledgePreview
                  products={data.products}
                  request={request}
                  onIntent={onIntent}
                />
              )}
              {tab === "behavior" && (
                <BehaviorView
                  request={request}
                  onProduct={onProduct}
                  onEntity={onEntity}
                />
              )}
              {tab === "experiments" && <ExperimentStudio request={request} />}
              {tab === "scenarios" && (
                <ScenarioStudio
                  request={request}
                  initialScenarioId={
                    new URLSearchParams(location.search).get("scenarioId") ||
                    undefined
                  }
                  sourceTaskId={
                    new URLSearchParams(location.search).get(
                      "scenarioSource",
                    ) || undefined
                  }
                  channel={
                    new URLSearchParams(location.search).get(
                      "scenarioChannel",
                    ) || "default"
                  }
                />
              )}
              {tab === "guardrails" && (
                <>
                  <SovereignAI request={request} />
                  <GuardrailSettings
                    request={request}
                    products={data.products}
                    onDirtyChange={setDirty}
                  />
                </>
              )}
              {tab === "observations" && (
                <>
                  <div className="knowledge-census">
                    {(
                      [
                        "productViews",
                        "cartAdds",
                        "reviews",
                        "processedEvents",
                      ] as const
                    ).map((field, i) => (
                      <div key={field}>
                        <span>
                          {k(
                            (
                              [
                                "views",
                                "cartAdds",
                                "reviews",
                                "processed",
                              ] as const
                            )[i],
                          )}
                        </span>
                        <strong>{number(workspace.totals[field] ?? 0)}</strong>
                      </div>
                    ))}
                  </div>
                  <p className="muted">{k("viewsHint")}</p>
                  <EvidenceReview
                    products={data.products}
                    workspace={workspace}
                    request={request}
                  />
                  <MemoryView
                    request={request}
                    products={data.products}
                    canWrite={workspace.canWrite}
                    onChange={() => void load()}
                  />
                </>
              )}
            </>
          )}
        </div>
      </div>
    </div>
  );
}
