/** Durable co-purchase evidence and revision-bound merchant decisions, with simulation labels and explicit confirmation. */
import {
  useRead,
  useSetRead,
} from "../../../../../frontend/src/admin/shell/useRead";
import { Button, Card, Notice } from "../../../../../frontend/src/kit/ui";
import { useState } from "react";
import { useKnowledgeText } from "../../../../../frontend/src/shared/i18n/knowledge-i18n";
import { useLocale } from "../../../../../frontend/src/shared/i18n/i18n";
import ConfirmDialog from "../../../../../frontend/src/shared/ui/ConfirmDialog";
import Icon from "../../../../../frontend/src/shared/ui/Icon";
import type {
  Product,
  RequestFn,
} from "../../../../../frontend/src/admin/shell/studio-types";
type DecisionState = "published" | "experiment" | "dismissed";
type Memory = {
  processedEvents: number;
  pairs: {
    left: string;
    right: string;
    orders: number;
    simulatedOrders: number;
    lastEvent: number;
    updatedAt: string;
  }[];
  hypotheses: {
    id: string;
    state: string;
    revision: number;
    evidence: { left: string; right: string; observedOrders: number };
  }[];
};
export default function MemoryView({
  request,
  products = [],
  canWrite = false,
  onChange,
}: {
  request: RequestFn;
  products?: Product[];
  canWrite?: boolean;
  onChange?: () => void;
}) {
  const k = useKnowledgeText(),
    { number, locale } = useLocale();
  const [error, setError] = useState(""),
    [busy, setBusy] = useState(false),
    [notice, setNotice] = useState("");
  const dataPath = "/api/intelligence";
  const dataRead = useRead<Memory>(request, dataPath);
  const setReadData = useSetRead(request);
  const data = dataRead.data ?? undefined;
  const dataLoadError = (dataRead.error as Error | undefined)?.message;

  const [decision, setDecision] = useState<{
    hypothesis: Memory["hypotheses"][number];
    state: DecisionState;
  }>();
  const name = (id: string) => products.find((p) => p.id === id)?.name ?? id;
  const label = (state: string) =>
    k(
      state === "published"
        ? "approved"
        : state === "experiment"
          ? "experiment"
          : state === "dismissed"
            ? "dismissed"
            : "proposed",
    );
  return (
    <Card className="knowledge-memory">
      <span className="kicker">
        {k("processed")} · {number(data?.processedEvents ?? 0)}
      </span>
      <h2>{k("observations")}</h2>
      <p>{k("learningHint")}</p>
      {!data?.pairs.length && (
        <p className="knowledge-empty">{k("noEvidence")}</p>
      )}
      {data?.pairs.map((p) => (
        <article className="knowledge-pair" key={`${p.left}:${p.right}`}>
          <div className="knowledge-pair-title">
            <Icon name="link" />
            <h3>
              {name(p.left)} <span>↔</span> {name(p.right)}
            </h3>
          </div>
          <div className="knowledge-pair-metrics">
            <span>
              <strong>{number(p.orders)}</strong>
              {k("orders")}
            </span>
            <span>
              <strong>{number(p.simulatedOrders)}</strong>
              {k("simulated")}
            </span>
            <span>
              <strong>{number(p.orders - p.simulatedOrders)}</strong>
              {k("real")}
            </span>
          </div>
          <small>
            {k("source")} #{p.lastEvent} ·{" "}
            {new Intl.DateTimeFormat(locale, { dateStyle: "medium" }).format(
              new Date(p.updatedAt),
            )}
          </small>
          {data.hypotheses
            .filter(
              (h) => h.evidence.left === p.left && h.evidence.right === p.right,
            )
            .map((h) => (
              <div className="knowledge-hypothesis" key={h.id}>
                <span className="knowledge-badge">
                  {label(h.state)} · {k("revision")} {h.revision}
                </span>
                {canWrite && (
                  <div className="knowledge-actions">
                    {(["published", "experiment", "dismissed"] as const).map(
                      (state) => (
                        <Button
                          tone={state === "published" ? "primary" : "secondary"}
                          type="submit"
                          key={state}
                          disabled={busy || state === h.state}
                          onClick={() => setDecision({ hypothesis: h, state })}
                        >
                          {k(
                            state === "published"
                              ? "recommend"
                              : state === "experiment"
                                ? "mark"
                                : "dismiss",
                          )}
                        </Button>
                      ),
                    )}
                  </div>
                )}
              </div>
            ))}
        </article>
      ))}
      <p className="knowledge-boundary">{k("boundary")}</p>
      {notice && <p role="status">{notice}</p>}
      {(error || dataLoadError) && (
        <Notice tone="danger">{error || dataLoadError}</Notice>
      )}
      {decision && (
        <ConfirmDialog
          title={k("confirm")}
          disabled={busy}
          confirmLabel={k("confirm")}
          onCancel={() => {
            if (!busy) setDecision(undefined);
          }}
          onConfirm={async () => {
            setBusy(true);
            setError("");
            try {
              await request(
                `/api/intelligence/hypotheses/${decision.hypothesis.id}`,
                {
                  state: decision.state,
                  revision: decision.hypothesis.revision,
                  approve: true,
                },
                "PUT",
              );
              setReadData(dataPath, await request(dataPath));
              setNotice(k("changed"));
              setDecision(undefined);
              onChange?.();
            } catch (e) {
              setError((e as Error).message);
            } finally {
              setBusy(false);
            }
          }}
        >
          <p>
            {name(decision.hypothesis.evidence.left)} ↔{" "}
            {name(decision.hypothesis.evidence.right)}
          </p>
          <p>{k("decisionHint")}</p>
          <p>{label(decision.state)}</p>
        </ConfirmDialog>
      )}
    </Card>
  );
}
