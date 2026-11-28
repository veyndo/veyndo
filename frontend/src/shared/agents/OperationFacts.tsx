/** Shows the typed facts of a proposed operation from the operations graph; presentation only, admission stays on the server. */
import { useEffect, useState } from "react";
import { Badge, type Tone } from "../../kit/ui";
import { createApiClient } from "../../kit/api";
import { forApps, type ApiRequestFn } from "../api/types";
import "./operation-facts.css";
import {
  useOperationFactsText,
  type OperationFactsWord,
} from "./operation-facts-i18n";

/** The planner view of one operation, as `merchant.operations.list` returns it. */
export type PlannedOperation = {
  name: string;
  readOnly: boolean;
  cost: string;
  requiresApproval: boolean;
  facts: {
    permissions: string[];
    writes: boolean;
    reversibility: { kind: string; by?: string };
    risk: "low" | "medium" | "high";
    data: { entities: string[]; pii: boolean; money: boolean; stock: boolean };
    effects: string[];
    conditions: string[];
  };
};
export type OperationFactsIndex = Record<string, PlannedOperation>;

const loaded = new WeakMap<object, Promise<OperationFactsIndex>>();
/** Loads the operations graph once per Studio request function; a failed read shows no facts rather than guessing. */
export function useOperationFacts(request?: ApiRequestFn) {
  const [index, setIndex] = useState<OperationFactsIndex>({});
  useEffect(() => {
    if (!request) return;
    let current = true;
    let pending = loaded.get(request);
    if (!pending) {
      pending = createApiClient(forApps(request))
        .call("merchant.operations.list", { includeUnavailable: true })
        .then((result) =>
          Object.fromEntries(
            (
              (result as unknown as { operations?: PlannedOperation[] })
                .operations ?? []
            ).map((operation) => [operation.name, operation]),
          ),
        );
      pending.catch(() => loaded.delete(request));
      loaded.set(request, pending);
    }
    pending.then(
      (value) => current && setIndex(value),
      () => current && setIndex({}),
    );
    return () => {
      current = false;
    };
  }, [request]);
  return index;
}

const riskTone: Record<string, Tone> = {
  low: "neutral",
  medium: "warning",
  high: "danger",
};
const effectWords: string[] = [
  "email",
  "payment",
  "externalCall",
  "modelCall",
  "publication",
  "backgroundJob",
  "event",
];

/** The badges of one step; an operation without declared facts is announced as such. */
export default function OperationFacts({
  operation,
  known = true,
}: {
  operation?: PlannedOperation;
  known?: boolean;
}) {
  const t = useOperationFactsText();
  if (!operation)
    return known ? null : (
      <p className="operation-facts" role="note">
        <Badge tone="warning">{t("unknown")}</Badge>
      </p>
    );
  const { facts } = operation;
  const reversibility = !facts.writes
    ? t("readOnly")
    : facts.reversibility.kind === "compensatable"
      ? t("compensatable", { operation: facts.reversibility.by ?? "" })
      : t(
          facts.reversibility.kind === "irreversible"
            ? "irreversible"
            : "reversible",
        );
  return (
    <div
      className="operation-facts"
      role="group"
      aria-label={t("facts")}
      data-risk={facts.risk}
    >
      {operation.requiresApproval && (
        <Badge tone="warning">{t("approval")}</Badge>
      )}
      <Badge tone={riskTone[facts.risk] ?? "neutral"}>
        {t(`risk_${facts.risk}` as OperationFactsWord)}
      </Badge>
      <Badge
        tone={
          facts.writes && facts.reversibility.kind === "irreversible"
            ? "danger"
            : "neutral"
        }
      >
        {reversibility}
      </Badge>
      {facts.data.pii && <Badge>{t("pii")}</Badge>}
      {facts.data.money && <Badge>{t("money")}</Badge>}
      {facts.data.stock && <Badge>{t("stock")}</Badge>}
      {facts.effects
        .filter((effect) => effectWords.includes(effect))
        .map((effect) => (
          <Badge key={effect} tone={effect === "payment" ? "danger" : "accent"}>
            {t(effect as OperationFactsWord)}
          </Badge>
        ))}
      {facts.conditions.includes("sandbox") && <Badge>{t("sandbox")}</Badge>}
      {facts.conditions.includes("personalSession") && (
        <Badge>{t("personalSession")}</Badge>
      )}
      {facts.permissions.length > 0 && (
        <small>
          {t("permissions", { permissions: facts.permissions.join(", ") })}
        </small>
      )}
    </div>
  );
}
