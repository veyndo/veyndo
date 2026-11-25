/** Native app workspace: OAuth, provider settings, durable jobs and private sources. */
import type { ApiPath } from "../../kit/api";
import { Button, Field, Notice, TextInput } from "../../kit/ui";
import { useEffect, useState } from "react";
import { useConnectedText } from "../../shared/i18n/connected-i18n";
import type { RequestFn } from "../shell/studio-types";
import { useRead, useSetRead } from "../shell/useRead";
import { connectorDescriptors } from "./connector-descriptors";
export default function ConnectorPanel({
  app,
  request,
  manage,
}: {
  app: string;
  request: RequestFn;
  manage: boolean;
}) {
  const { x } = useConnectedText();
  const contract = connectorDescriptors[app];
  const [settings, setSettings] = useState<Record<string, any>>({});
  const [dirty, setDirty] = useState(false);
  const [actionError, setError] = useState("");
  const [busy, setBusy] = useState(false);
  const [choices, setChoices] = useState<any[]>([]);
  const call = (name: string, value: unknown = {}) =>
    request(`/api/apps/${app}/actions/${name}`, value);
  // The status action is a read; it polls while a connector job runs.
  const statusPath: ApiPath = `/api/apps/${app}/actions/status`;
  const [running, setRunning] = useState(false);
  const statusRead = useRead(request, statusPath, {
    body: {},
    refetchMs: running ? 1500 : undefined,
  });
  const evidenceRead = useRead(request, "/api/knowledge/external", {
    enabled: !!contract.sources,
  });
  const setRead = useSetRead(request);
  const status = statusRead.data;
  const sources: any[] = (evidenceRead.data?.elements ?? []).filter(
    (r: any) => r.app === app,
  );
  const error =
    actionError ||
    (statusRead.error as Error | undefined)?.message ||
    (evidenceRead.error as Error | undefined)?.message ||
    "";
  useEffect(() => {
    setRunning(
      !!status?.jobs.some((j: any) => ["queued", "running"].includes(j.state)),
    );
    if (status && !dirty) setSettings(status.settings);
  }, [status]);
  const refresh = async () => {
    await statusRead.refetch();
    if (contract.sources) await evidenceRead.refetch();
  };
  const run = async (fn: () => Promise<unknown>) => {
    setBusy(true);
    setError("");
    try {
      await fn();
      await refresh();
    } catch (e) {
      setError((e as Error).message);
    } finally {
      setBusy(false);
    }
  };
  const field = (key: string, label: string, placeholder = "") => (
    <Field label={x(label)}>
      <TextInput
        value={settings[key] ?? ""}
        placeholder={placeholder}
        disabled={!manage || busy}
        onChange={(e) => {
          setDirty(true);
          setSettings((s) => ({ ...s, [key]: e.target.value }));
        }}
      />
    </Field>
  );
  return (
    <div className="connector-panel">
      <header>
        <div>
          <span className="kicker">{x("connection")}</span>
          <h3>{x(status?.connected ? "connected" : "disconnected")}</h3>
        </div>
        <Button type="button" disabled={busy} onClick={() => void run(refresh)}>
          {x("refresh")}
        </Button>
      </header>
      {error && <Notice tone="danger">{error}</Notice>}
      {status && !status.oauthClientConfigured && <p>{x("setup")}</p>}
      <p>{x(contract.hint)}</p>
      <div className="workbench-row">
        <Button
          tone="primary"
          type="button"
          disabled={!manage || busy || !status?.oauthClientConfigured}
          onClick={() => {
            const popup = window.open("about:blank", "_blank");
            if (popup) popup.opener = null;
            void run(async () => {
              try {
                const v = await call("connect");
                if (popup) popup.location.href = v.authorizationUrl;
              } catch (e) {
                popup?.close();
                throw e;
              }
            });
          }}
        >
          {x("connect")}
        </Button>
        {status?.connected && (
          <Button
            type="button"
            disabled={!manage || busy}
            onClick={() => void run(() => call("disconnect"))}
          >
            {x("disconnect")}
          </Button>
        )}
      </div>
      <div className="connector-settings">
        {contract.fields.map((f) => (
          <span key={f.key}>{field(f.key, f.label, f.placeholder)}</span>
        ))}
        {contract.toggle && (
          <label className="checkbox-label">
            <input
              type="checkbox"
              disabled={!manage || busy}
              checked={settings[contract.toggle.key] ?? false}
              onChange={(e) => {
                setDirty(true);
                setSettings((s) => ({
                  ...s,
                  [contract.toggle!.key]: e.target.checked,
                }));
              }}
            />
            {x(contract.toggle.label)}
          </label>
        )}
        {contract.choices && status?.connected && (
          <>
            <Button
              type="button"
              disabled={!manage || busy}
              onClick={() =>
                void run(async () => {
                  const v = await call(contract.choices!.action);
                  setChoices(v.channels ?? v.labels ?? []);
                })
              }
            >
              {x("choose")}
            </Button>
            {choices.length > 0 && (
              <select
                aria-label={x(contract.choices.label)}
                value={settings[contract.choices.key] ?? ""}
                onChange={(e) => {
                  setDirty(true);
                  setSettings((s) => ({
                    ...s,
                    [contract.choices!.key]: e.target.value,
                  }));
                }}
              >
                <option value="">—</option>
                {choices.map((c) => (
                  <option key={c.id} value={c.id}>
                    {c.name}
                  </option>
                ))}
              </select>
            )}
          </>
        )}
        <Button
          tone="primary"
          type="button"
          disabled={!manage || busy || !status}
          onClick={() =>
            void run(async () => {
              const v = await call("configure", {
                revision: status.revision,
                settings,
              });
              setRead(statusPath, v, {});
              setSettings(v.settings);
              setDirty(false);
            })
          }
        >
          {x("save")}
        </Button>
        {contract.sync && (
          <Button
            type="button"
            disabled={!manage || busy || !status?.connected}
            onClick={() =>
              void run(() => call("sync", { requestKey: crypto.randomUUID() }))
            }
          >
            {x("sync")}
          </Button>
        )}
      </div>
      {contract.sources && (
        <section>
          <h3>{x("private")}</h3>
          <p>{x("privacy")}</p>
          {!sources.length ? (
            <p>{x("empty")}</p>
          ) : (
            sources.map((s) => (
              <a
                className="search-hit"
                key={s.sourceId}
                href={s.sourceUrl}
                target="_blank"
                rel="noreferrer"
              >
                {s.title}
                <small>{s.sourceId}</small>
              </a>
            ))
          )}
        </section>
      )}
      <section>
        <h3>{x("jobs")}</h3>
        {status?.jobs.map((j: any) => (
          <div className="connector-job" key={j.id}>
            <strong>{x(j.state)}</strong>
            <small>{j.id}</small>
          </div>
        ))}
      </section>
    </div>
  );
}
