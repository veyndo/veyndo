/** Personal, expiring, least-privilege integration keys; plaintext stays in component memory and is never reloaded. */
import {
  useRead,
  useSetRead,
} from "../../../../../../frontend/src/admin/shell/useRead";
import {
  Button,
  Card,
  Field,
  Notice,
  Select,
  TextInput,
} from "../../../../../../frontend/src/kit/ui";
import "../api-console.css";
import { useEffect, useRef, useState } from "react";
import type { RequestFn } from "../../../../../../frontend/src/admin/shell/studio-types";
import PermissionPicker from "../../../../../identity/ui/admin/team/PermissionPicker";
import ConfirmDialog from "../../../../../../frontend/src/shared/ui/ConfirmDialog";
import { usePermissionText } from "../../../../../identity/ui/admin/team/permission-i18n";
import { useApiText } from "./api-i18n";
import AgentBudgetFields, {
  agentBudget,
  planningScope,
  type BudgetFields,
} from "./AgentBudgetFields";
import { useAgentControlText } from "./agent-control-i18n";
import { useLocale } from "../../../../../../frontend/src/shared/i18n/i18n";
type Key = {
  id: string;
  name: string;
  permissions: string[];
  expiresAt: string | null;
  agentBudget?: ReturnType<typeof agentBudget>;
  agentUsage?: { tokensAllocated: number; costMicrosAllocated: number };
};
export default function IntegrationKeys({ request }: { request: RequestFn }) {
  const t = useApiText();
  const at = useAgentControlText();
  const { locale } = useLocale();
  const usd = (n: number) =>
    new Intl.NumberFormat(locale, {
      style: "currency",
      currency: "USD",
      maximumFractionDigits: 6,
    }).format(n / 1_000_000);
  const current = useRef(request);
  current.current = request;
  const secretOwner = useRef<RequestFn | null>(null);
  const [purpose, setPurpose] = useState("api"),
    [budgetFields, setBudgetFields] = useState<BudgetFields>({
      provider: "",
      model: "",
      tokens: "",
      cost: "",
      tariff: "",
    });
  const budget = agentBudget(budgetFields);
  const pt = usePermissionText();
  const accessRead = useRead(request, "/api/auth/access");
  const access: string[] = accessRead.data?.permissions ?? [];
  const personal = accessRead.data?.personalSession === true;
  const integrations = useRead(
    request,
    access.includes("team.manage") ? "/api/workspace/integrations" : null,
  );
  const setRead = useSetRead(request);
  const keys: Key[] = integrations.data?.elements ?? [];
  const [name, setName] = useState(""),
    [days, setDays] = useState(30),
    [permanent, setPermanent] = useState(false),
    [scopes, setScopes] = useState<string[]>([]),
    [secret, setSecret] = useState(""),
    [actionError, setError] = useState(""),
    [busy, setBusy] = useState(false),
    [revoke, setRevoke] = useState<Key | null>(null),
    [copied, setCopied] = useState(false);
  const manage = access.includes("team.manage");
  const error =
    actionError ||
    (accessRead.error as Error | undefined)?.message ||
    (integrations.error as Error | undefined)?.message ||
    "";
  const load = async () => {
    const result = await request("/api/workspace/integrations");
    setRead("/api/workspace/integrations", result);
  };
  useEffect(() => {
    setSecret("");
    setScopes([]);
    setBusy(false);
    setRevoke(null);
    setError("");
  }, [request]);
  return (
    <Card className="api-key-panel">
      <h2>{t("keys")}</h2>
      <p>{t("manageHint")}</p>
      <form
        onSubmit={async (e) => {
          e.preventDefault();
          setBusy(true);
          setError("");
          setSecret("");
          try {
            const result = await request(
              "/api/workspace/integrations",
              {
                name: name.trim(),
                expiresInDays: days,
                permissions: scopes,
                ...(permanent ? { nonExpiring: true } : {}),
                ...(purpose === "agent" ? { agentBudget: budget } : {}),
              },
              "POST",
            );
            if (current.current !== request) return;
            secretOwner.current = request;
            setSecret(result.key);
            setCopied(false);
            setName("");
            setScopes([]);
            await load();
          } catch (e) {
            if (current.current === request) setError((e as Error).message);
          } finally {
            if (current.current === request) setBusy(false);
          }
        }}
      >
        <fieldset disabled={!manage || busy}>
          <div className="api-key-fields">
            <Field label={at("kind")}>
              <Select
                value={purpose}
                onChange={(e) => {
                  setPurpose(e.target.value);
                  if (e.target.value === "agent")
                    setScopes(scopes.filter(planningScope));
                }}
              >
                <option value="api">{at("api")}</option>
                {personal && <option value="agent">{at("agent")}</option>}
              </Select>
            </Field>
            <Field label={t("name")}>
              <TextInput
                required
                maxLength={100}
                value={name}
                onChange={(e) => setName(e.target.value)}
              />
            </Field>
            <Field label={t("lifetime")}>
              <Select
                value={permanent ? "permanent" : "limited"}
                onChange={(e) => setPermanent(e.target.value === "permanent")}
              >
                <option value="limited">{t("limited")}</option>
                {personal && (
                  <option value="permanent">{t("permanent")}</option>
                )}
              </Select>
            </Field>
            {!permanent && (
              <Field label={t("days")}>
                <TextInput
                  required
                  type="number"
                  min={1}
                  max={90}
                  step={1}
                  value={Number.isFinite(days) ? days : ""}
                  onChange={(e) => setDays(e.target.valueAsNumber)}
                />
              </Field>
            )}
          </div>
          {permanent && <p>{t("permanentHint")}</p>}
          {purpose === "agent" && (
            <AgentBudgetFields
              value={budgetFields}
              onChange={setBudgetFields}
            />
          )}
          <h3>{t("rights")}</h3>
          <PermissionPicker
            available={access.filter(
              (p) => p !== "*" && (purpose !== "agent" || planningScope(p)),
            )}
            value={scopes}
            onChange={setScopes}
          />
          <Button
            tone="primary"
            type="submit"
            disabled={
              !name.trim() ||
              !scopes.length ||
              (purpose === "agent" && (!personal || !budget)) ||
              (!permanent && (!Number.isInteger(days) || days < 1 || days > 90))
            }
          >
            {t("create")}
          </Button>
        </fieldset>
      </form>
      {secret && secretOwner.current === request && (
        <div className="api-secret">
          <p>{t("once")}</p>
          <input
            readOnly
            type="password"
            aria-label={t("keys")}
            value={secret}
            autoComplete="off"
          />
          <Button
            type="submit"
            onClick={async () => {
              try {
                await navigator.clipboard.writeText(secret);
                setCopied(true);
              } catch (e) {
                setError((e as Error).message);
              }
            }}
          >
            {t(copied ? "copied" : "copy")}
          </Button>
          <Button type="submit" onClick={() => setSecret("")}>
            {t("clear")}
          </Button>
        </div>
      )}
      {keys.map((key) => (
        <article className="api-key-row" key={key.id}>
          <div>
            <strong>{key.name}</strong>
            {key.agentBudget && (
              <p>
                {at("agent")} · {key.agentBudget.model} · {at("allocated")}:{" "}
                {key.agentUsage?.tokensAllocated ?? 0} /{" "}
                {key.agentBudget.tokens} ·{" "}
                {usd(key.agentUsage?.costMicrosAllocated ?? 0)} /{" "}
                {usd(key.agentBudget.costMicros)}
              </p>
            )}
            <p>{key.permissions.map((p) => `${pt(p)} (${p})`).join(" · ")}</p>
            <small>
              {t("expiry")}:{" "}
              {key.expiresAt
                ? new Date(key.expiresAt).toLocaleString()
                : t("permanent")}
              {key.expiresAt && new Date(key.expiresAt).getTime() <= Date.now()
                ? ` · ${t("expired")}`
                : ""}
            </small>
          </div>
          <Button
            type="submit"
            disabled={!manage || busy}
            onClick={() => setRevoke(key)}
          >
            {t("revoke")}
          </Button>
        </article>
      ))}
      {revoke && (
        <ConfirmDialog
          title={t("revoke")}
          confirmLabel={t("revoke")}
          disabled={busy}
          onCancel={() => setRevoke(null)}
          onConfirm={async () => {
            setBusy(true);
            setError("");
            try {
              await request(
                `/api/workspace/integrations/${encodeURIComponent(revoke.id)}`,
                undefined,
                "DELETE",
              );
              if (current.current !== request) return;
              setRevoke(null);
              await load();
            } catch (e) {
              if (current.current === request) setError((e as Error).message);
            } finally {
              if (current.current === request) setBusy(false);
            }
          }}
        >
          <strong>{revoke.name}</strong>
          <p>{t("revokeHint")}</p>
        </ConfirmDialog>
      )}
      {error && <Notice tone="danger">{error}</Notice>}
    </Card>
  );
}
