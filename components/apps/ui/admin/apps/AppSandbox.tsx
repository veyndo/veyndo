/** Per-app sandbox transparency: declared permissions, network access, data, ports and agent tool effects beside the recorded calls, service calls and data changes, with explicit version rollback. */
import { useState, type ReactNode } from "react";
import type { ApiPath } from "../../../../../frontend/src/kit/api";
import {
  useRead,
  useSetRead,
} from "../../../../../frontend/src/admin/shell/useRead";
import { Button, Notice } from "../../../../../frontend/src/kit/ui";
import type { RequestFn } from "../../../../../frontend/src/admin/shell/studio-types";
import ConfirmDialog from "../../../../../frontend/src/shared/ui/ConfirmDialog";
import { useSandboxText, type SandboxWord } from "./sandbox-i18n";

type Facts = {
  audience: "shopper" | "merchant";
  permissions: string[];
  writes: boolean;
  reversibility: {
    kind: "reversible" | "compensatable" | "irreversible";
    by?: string;
  };
  risk: "low" | "medium" | "high";
  data: { entities: string[]; pii: boolean; money: boolean; stock: boolean };
  effects: string[];
  requires: string[];
  conditions: string[];
};
export type Sandbox = {
  app: string;
  version: string;
  publisher: { publisher: string; keyId: string; channel: string } | null;
  declared: {
    permissions: string[];
    egress: { service: boolean; approved: boolean; origins: string[] };
    data: { entity: string; fields: number; public: boolean }[];
    ports: {
      provides: { name: string; version: string; action: string }[];
      consumes: {
        name: string;
        version: string;
        provider: { app: string; version: string } | null;
      }[];
    };
    tools: {
      name: string;
      declared: boolean;
      facts: Facts;
      cost: string;
      requiresApproval: boolean;
    }[];
  };
  observed: {
    calls: { id: number; action: string; status: number; at: string }[];
    egress: { id: number; action: string; status: number; at: string }[];
    writes: {
      entity: string;
      record: string;
      revision: number;
      source: string;
      at: string;
    }[];
  };
  versions: {
    version: string;
    digest: string;
    installedAt: string;
    current: boolean;
    rollback: boolean;
  }[];
};

function List({ items, empty }: { items: ReactNode[]; empty: string }) {
  return items.length ? <ul>{items}</ul> : <p>{empty}</p>;
}

export default function AppSandbox({
  app,
  request,
}: {
  app: string;
  request: RequestFn;
}) {
  const t = useSandboxText();
  const path: ApiPath = `/api/apps/${app}/sandbox`;
  const read = useRead<Sandbox | null>(request, path);
  const setRead = useSetRead(request);
  const [target, setTarget] = useState<string | null>(null),
    [busy, setBusy] = useState(false),
    [error, setError] = useState(""),
    [done, setDone] = useState("");
  const data = read.data ?? null;
  const loadError = (read.error as Error | undefined)?.message;
  const rollback = async (version: string) => {
    setBusy(true);
    setError("");
    try {
      await request(`/api/apps/${app}/rollback`, { version, approve: true });
      setDone(t("rolledBack", { version }));
      setTarget(null);
      setRead(path, await request(path));
    } catch (e) {
      setError((e as Error).message);
    } finally {
      setBusy(false);
    }
  };
  // Render only a complete sandbox document; anything else is a load problem, not an empty sandbox.
  if (!data?.declared || !data.observed || !data.versions)
    return loadError ? <Notice tone="danger">{loadError}</Notice> : null;
  const { declared, observed } = data;
  const time = (at: string) => (
    <time dateTime={at}>{new Date(at).toLocaleString()}</time>
  );
  return (
    <section className="app-contract-summary app-sandbox">
      <header className="app-detail-heading">
        <h3>{t("title")}</h3>
        <small>
          {data.publisher
            ? `${t("signed")} ${data.publisher.publisher} · ${data.publisher.channel}`
            : t("unsigned")}
        </small>
      </header>
      <p>{t("intro")}</p>
      <div>
        <div>
          <h4>{t("mayDo")}</h4>
          <details open>
            <summary>
              {t("permissions")} · {declared.permissions.length}
            </summary>
            <List
              empty={t("noPermissions")}
              items={declared.permissions.map((p) => (
                <li key={p}>
                  <code>{p}</code>
                </li>
              ))}
            />
          </details>
          <details open>
            <summary>{t("egress")}</summary>
            {declared.egress.service ? (
              <>
                <p>
                  {declared.egress.approved ? t("service") : t("notApproved")}
                </p>
                <List
                  empty=""
                  items={declared.egress.origins.map((o) => (
                    <li key={o}>
                      <code>{o}</code>
                    </li>
                  ))}
                />
              </>
            ) : (
              <p>{t("noEgress")}</p>
            )}
          </details>
          <details>
            <summary>
              {t("dataScopes")} · {declared.data.length}
            </summary>
            <List
              empty={t("nothing")}
              items={declared.data.map((d) => (
                <li key={d.entity}>
                  <code>{d.entity}</code> · {d.fields} {t("fields")} ·{" "}
                  {d.public ? t("publicModel") : t("privateModel")}
                </li>
              ))}
            />
          </details>
          {(declared.ports.provides.length > 0 ||
            declared.ports.consumes.length > 0) && (
            <details open>
              <summary>{t("ports")}</summary>
              <ul>
                {declared.ports.provides.map((p) => (
                  <li key={`p-${p.name}`}>
                    {t("provides")} <code>{p.name}</code> {p.version}
                  </li>
                ))}
                {declared.ports.consumes.map((p) => (
                  <li key={`c-${p.name}`}>
                    {t("consumes")} <code>{p.name}</code> {p.version} ·{" "}
                    {p.provider
                      ? `${t("wiredTo")} ${p.provider.app} ${p.provider.version}`
                      : t("unwired")}
                  </li>
                ))}
              </ul>
            </details>
          )}
          <details>
            <summary>
              {t("tools")} · {declared.tools.length}
            </summary>
            <List
              empty={t("nothing")}
              items={declared.tools.map((tool) => (
                <li key={tool.name}>
                  <code>{tool.name}</code> ·{" "}
                  {t(`risk_${tool.facts.risk}` as SandboxWord)} ·{" "}
                  {t(`rev_${tool.facts.reversibility.kind}` as SandboxWord)} ·{" "}
                  {tool.requiresApproval ? t("approval") : t("noApproval")}
                  {!tool.declared && <small> · {t("defaults")}</small>}
                </li>
              ))}
            />
          </details>
        </div>
        <div>
          <h4>{t("didDo")}</h4>
          <details open>
            <summary>
              {t("calls")} · {observed.calls.length}
            </summary>
            <List
              empty={t("nothing")}
              items={observed.calls.map((c) => (
                <li key={c.id}>
                  <code>{c.action}</code> · {c.status} · {time(c.at)}
                </li>
              ))}
            />
          </details>
          <details>
            <summary>
              {t("egressCalls")} · {observed.egress.length}
            </summary>
            <List
              empty={t("nothing")}
              items={observed.egress.map((c) => (
                <li key={c.id}>
                  <code>{c.action}</code> · {c.status} · {time(c.at)}
                </li>
              ))}
            />
          </details>
          <details>
            <summary>
              {t("writes")} · {observed.writes.length}
            </summary>
            <List
              empty={t("nothing")}
              items={observed.writes.map((w) => (
                <li key={`${w.entity}-${w.record}-${w.revision}`}>
                  <code>
                    {w.entity}/{w.record}
                  </code>{" "}
                  · #{w.revision} · {w.source} · {time(w.at)}
                </li>
              ))}
            />
          </details>
        </div>
      </div>
      <details>
        <summary>
          {t("versions")} · {data.versions.length}
        </summary>
        <ul>
          {data.versions.map((v) => (
            <li key={v.version}>
              <code>{v.version}</code> · {time(v.installedAt)}
              {v.current ? (
                <strong> · {t("current")}</strong>
              ) : v.rollback ? (
                <Button
                  type="button"
                  disabled={busy}
                  onClick={() => setTarget(v.version)}
                >
                  {t("rollback")}
                </Button>
              ) : (
                <small> · {t("noRollback")}</small>
              )}
            </li>
          ))}
        </ul>
      </details>
      {done && <p role="status">{done}</p>}
      {error && <Notice tone="danger">{error}</Notice>}
      {target && (
        <ConfirmDialog
          title={t("rollbackTitle")}
          confirmLabel={t("rollback")}
          disabled={busy}
          onCancel={() => setTarget(null)}
          onConfirm={() => void rollback(target)}
        >
          <p>{t("rollbackWarning", { version: target })}</p>
        </ConfirmDialog>
      )}
    </section>
  );
}
