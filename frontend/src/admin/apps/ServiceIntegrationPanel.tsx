/** Revisioned encrypted connection forms and bounded operations on the existing app action gateway. */
import { Button, Field, Notice, TextInput } from "../../kit/ui";
import { useEffect, useRef, useState } from "react";
import type { RequestFn } from "../shell/studio-types";
import { useRead, useSetRead } from "../shell/useRead";
import { serviceIntegrations } from "./service-integrations";
import {
  useIntegrationText,
  type IntegrationKey,
} from "./service-integration-text";
import "./service-integrations.css";
type Status = {
  revision: number;
  settings: Record<string, string | boolean>;
  credentialConfigured: boolean;
  jobs: {
    id: string;
    state: string;
    result?: { state?: string; nextOffset?: number };
  }[];
};
export default function ServiceIntegrationPanel({
  app,
  request,
  manage,
}: {
  app: string;
  request: RequestFn;
  manage: boolean;
}) {
  const t = useIntegrationText(),
    d = serviceIntegrations[app];
  const [settings, setSettings] = useState<Record<string, string | boolean>>({
      enabled: false,
    }),
    [credential, setCredential] = useState(""),
    [input, setInput] = useState<Record<string, string>>({}),
    [records, setRecords] = useState<unknown[]>([]),
    [actionError, setError] = useState(""),
    [busy, setBusy] = useState(false);
  const generation = useRef(0);
  const fileRead = useRef(0);
  const path = `/api/apps/${app}/actions/`;
  // The status action is a read; only managers load it.
  const statusRead = useRead<Status>(request, path + "status", {
    body: {},
    enabled: manage,
  });
  const setRead = useSetRead(request);
  const status = statusRead.data;
  const error =
    actionError || (statusRead.error as Error | undefined)?.message || "";
  const dirty =
    !!credential ||
    JSON.stringify({ enabled: false, ...settings }) !==
      JSON.stringify({ enabled: false, ...status?.settings });
  useEffect(() => {
    ++generation.current;
    fileRead.current++;
    setBusy(false);
    setCredential("");
    setInput({});
    setRecords([]);
    setError("");
    return () => {
      generation.current++;
    };
  }, [app, request, manage, path]);
  // The first status of an app fills the settings form; later ones keep drafts.
  const seeded = useRef("");
  useEffect(() => {
    if (!status || seeded.current === path) return;
    seeded.current = path;
    setSettings({ enabled: false, ...status.settings });
  }, [status, path]);
  async function run(action: string, value: unknown, replace = false) {
    const g = generation.current;
    setBusy(true);
    setError("");
    try {
      await request(path + action, value);
      const v = await request(path + "status", {});
      if (g === generation.current) {
        setRead(path + "status", v, {});
        if (replace) {
          setSettings({ enabled: false, ...v.settings });
          setCredential("");
        }
      }
    } catch (e) {
      if (g === generation.current) setError((e as Error).message);
    } finally {
      if (g === generation.current) setBusy(false);
    }
  }
  if (!manage) return <p>{t("readonly")}</p>;
  return (
    <section className="service-integration">
      <h2>{d.name}</h2>
      {error && <Notice tone="danger">{error}</Notice>}
      <div className="service-integration-columns">
        <form
          onSubmit={(e) => {
            e.preventDefault();
            void run(
              "configure",
              {
                revision: status?.revision,
                settings,
                credentials: credential ? { apiKey: credential } : {},
              },
              true,
            );
          }}
        >
          <h3>{t("settings")}</h3>
          <fieldset disabled={busy || !status}>
            <label className="service-integration-enabled">
              <input
                type="checkbox"
                checked={settings.enabled === true}
                onChange={(e) =>
                  setSettings({ ...settings, enabled: e.target.checked })
                }
              />
              {t("enabled")}
            </label>
            {d.fields.map((key) => (
              <label key={key}>
                {t(key as IntegrationKey)}
                <input
                  value={String(settings[key] ?? "")}
                  onChange={(e) =>
                    setSettings({ ...settings, [key]: e.target.value })
                  }
                  maxLength={100}
                />
              </label>
            ))}
            <Field label={t("credential")}>
              <TextInput
                type="password"
                autoComplete="new-password"
                value={credential}
                onChange={(e) => setCredential(e.target.value)}
                maxLength={4000}
              />
            </Field>
            <small>{t("protected")}</small>
            <p>{t(status?.credentialConfigured ? "saved" : "missing")}</p>
            <Button tone="primary" type="submit">
              {t("save")}
            </Button>
          </fieldset>
        </form>
        <form
          onSubmit={(e) => {
            e.preventDefault();
            const fields: Record<string, unknown> = {};
            for (const key of d.operation) {
              if (key === "properties") continue;
              if (key === "records") fields.records = records;
              else if (key === "offset") {
                if (input.offset) fields.offset = Number(input.offset);
              } else fields[key] = input[key] ?? "";
            }
            void run("run", { requestKey: crypto.randomUUID(), input: fields });
          }}
        >
          <h3>{t("run")}</h3>
          <p>{t("scope")}</p>
          {dirty && <p role="status">{t("saveFirst")}</p>}
          <fieldset
            disabled={
              busy ||
              !status ||
              settings.enabled !== true ||
              !status.credentialConfigured ||
              dirty
            }
          >
            {d.operation
              .filter((k) => k !== "properties")
              .map((key) =>
                key === "records" ? (
                  <label key={key}>
                    {t("file")}
                    <input
                      type="file"
                      accept="application/json,.json"
                      onChange={async (e) => {
                        setRecords([]);
                        const g = generation.current;
                        const selection = ++fileRead.current;
                        const file = e.target.files?.[0];
                        if (!file) return;
                        if (file.size > 40000) {
                          setError(t("fileError"));
                          return;
                        }
                        try {
                          const raw = await file.text();
                          if (
                            g !== generation.current ||
                            selection !== fileRead.current
                          )
                            return;
                          const value = JSON.parse(raw);
                          if (
                            !Array.isArray(value) ||
                            !value.length ||
                            value.length > 50
                          )
                            throw Error();
                          setRecords(value);
                          setError("");
                        } catch {
                          if (
                            g === generation.current &&
                            selection === fileRead.current
                          )
                            setError(t("fileError"));
                        }
                      }}
                    />
                    <small>
                      {t("records")}: {records.length}
                    </small>
                  </label>
                ) : (
                  <label key={key}>
                    {t(key as IntegrationKey)}
                    <input
                      type={
                        key === "email"
                          ? "email"
                          : key === "offset"
                            ? "number"
                            : "text"
                      }
                      value={input[key] ?? ""}
                      required={key !== "offset"}
                      onChange={(e) =>
                        setInput({ ...input, [key]: e.target.value })
                      }
                    />
                  </label>
                ),
              )}
            <Button
              tone="primary"
              type="submit"
              disabled={app === "algolia" && !records.length}
            >
              {t("run")}
            </Button>
          </fieldset>
        </form>
      </div>
      <header>
        <h3>{t("jobs")}</h3>
        <Button
          type="submit"
          disabled={busy || !status}
          onClick={() => void run("status", {})}
        >
          {t("refresh")}
        </Button>
      </header>
      {!status?.jobs.length && <p>{t("empty")}</p>}
      <ul>
        {status?.jobs.map((j) => (
          <li key={j.id}>
            <code>{j.id}</code>
            <span>
              {t(
                ([
                  "queued",
                  "running",
                  "completed",
                  "uncertain",
                  "failed",
                ].includes(j.state)
                  ? j.state
                  : "uncertain") as IntegrationKey,
              )}
            </span>
            {j.result?.state === "accepted" && <small>{t("accepted")}</small>}
            {j.result?.nextOffset !== undefined &&
              j.result.nextOffset !== null && (
                <small>
                  {t("offset")}: {j.result.nextOffset}
                </small>
              )}
          </li>
        ))}
      </ul>
    </section>
  );
}
