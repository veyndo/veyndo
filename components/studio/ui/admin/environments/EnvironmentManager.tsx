/** Private environment creation and digest-bound selective release. */
import type { ApiPath } from "../../../../../frontend/src/kit/api";
import {
  useRead,
  useSetRead,
} from "../../../../../frontend/src/admin/shell/useRead";
import {
  Button,
  Card,
  Field,
  LinkButton,
  Notice,
  Select,
  TextInput,
} from "../../../../../frontend/src/kit/ui";
import { useEffect, useMemo, useRef, useState } from "react";
import ReleaseScenarios from "./ReleaseScenarios";
import { useWorkbenchText } from "../../../../../frontend/src/shared/i18n/workbench-i18n";
import { contentText } from "../../../../../frontend/src/shared/i18n/content-language";
import type { RequestFn } from "../../../../../frontend/src/admin/shell/studio-types";
export type Environment = { id: string; name: string };
type Change = {
  key: string;
  before: unknown;
  after: unknown;
  digest: string;
  conflict: boolean;
};
export default function EnvironmentManager({
  request,
  environments,
  onRefresh,
  role,
  rights,
  onSelect,
  initialId,
  sandboxRequest,
  onTest,
}: {
  request: RequestFn;
  environments: Environment[];
  onRefresh: () => Promise<void>;
  role: string;
  rights?: string[];
  onSelect: (id: string) => void;
  initialId?: string;
  sandboxRequest?: (environment: string) => RequestFn;
  onTest?: (
    environment: string,
    channel: string,
    scenario?: string,
  ) => void | Promise<void>;
}) {
  const { w, date, locale } = useWorkbenchText();
  const [name, setName] = useState("");
  const [id, setId] = useState(
    environments.some((e) => e.id === initialId)
      ? initialId!
      : (environments[0]?.id ?? ""),
  );
  const [selected, setSelected] = useState<string[]>([]);
  const [actionError, setError] = useState("");
  const [busy, setBusy] = useState(false);
  const [notice, setNotice] = useState(false);
  const scope = useMemo(() => ({}), [request, id]);
  const currentScope = useRef(scope);
  currentScope.current = scope;
  const diffPath: ApiPath | null = id ? `/api/environments/${id}/diff` : null;
  const releases = useRead(request, "/api/environments");
  const diff = useRead(request, diffPath);
  const setRead = useSetRead(request);
  const history: {
    id: string;
    environment: string;
    selections: string[];
    createdAt: string;
  }[] = releases.data?.releases ?? [];
  const changes: Change[] = diff.data?.changes ?? [];
  const mainLocale: string = diff.data?.mainLocale ?? "";
  const loaded = !!diff.data;
  const visibleChanges = changes;
  const visibleSelected = selected;
  // A new comparison clears the selection.
  useEffect(() => setSelected([]), [diff.data]);
  const error =
    actionError ||
    (releases.error as Error | undefined)?.message ||
    (diff.error as Error | undefined)?.message ||
    "";
  const manage = rights
    ? rights.includes("environments.manage")
    : ["owner", "admin"].includes(role);
  const load = async () => {
    const data = await request("/api/environments");
    if (currentScope.current !== scope) return;
    setRead("/api/environments", data);
    if (diffPath) {
      const next = await request(diffPath);
      if (currentScope.current !== scope) return;
      setRead(diffPath, next);
    }
  };
  useEffect(() => {
    setError("");
    setNotice(false);
    setBusy(false);
  }, [request, id, scope]);
  const run = async (fn: () => Promise<void>) => {
    setBusy(true);
    setError("");
    setNotice(false);
    try {
      await fn();
    } catch (e) {
      if (currentScope.current === scope) setError((e as Error).message);
    } finally {
      if (currentScope.current === scope) setBusy(false);
    }
  };
  return (
    <div className="studio-page workbench">
      <div className="page-intro">
        <h1>{w("environments")}</h1>
        <p>{w("stageHint")}</p>
      </div>
      <Card>
        <form
          className="workbench-row"
          onSubmit={(e) => {
            e.preventDefault();
            void run(async () => {
              const env = await request("/api/environments", { name });
              await onRefresh();
              if (currentScope.current !== scope) return;
              setId(env.id);
              setName("");
            });
          }}
        >
          <Field label={w("stageName")}>
            <TextInput
              required
              maxLength={80}
              value={name}
              onChange={(e) => setName(e.target.value)}
            />
          </Field>
          <Button tone="primary" type="submit" disabled={busy || !manage}>
            {w("newStage")}
          </Button>
        </form>
        <p className="muted">{w("exclusion")}</p>
      </Card>
      {environments.length > 0 && (
        <Card>
          <div className="workbench-row">
            <Field label={w("stage")}>
              <Select value={id} onChange={(e) => setId(e.target.value)}>
                <option value="">{w("chooseStage")}</option>
                {environments.map((e) => (
                  <option key={e.id} value={e.id}>
                    {e.name}
                  </option>
                ))}
              </Select>
            </Field>
            <Button type="submit" disabled={!id} onClick={() => onSelect(id)}>
              {w("stage")}
            </Button>
            <LinkButton
              target="_blank"
              rel="noreferrer"
              href={`/?shop=${id}&sandbox=1#`}
            >
              {w("preview")} ↗
            </LinkButton>
            <Button
              type="submit"
              disabled={busy || !id}
              onClick={() => void run(load)}
            >
              {w("changes")}
            </Button>
          </div>
          {id && sandboxRequest && onTest && (
            <ReleaseScenarios
              environment={id}
              scope={scope}
              rights={rights ?? []}
              sandboxRequest={sandboxRequest}
              onOpen={onTest}
            />
          )}
          {loaded && !visibleChanges.length && <p>{w("noChanges")}</p>}
          {visibleChanges.map((c) => (
            <article className="release-change" key={c.key}>
              <label>
                <input
                  type="checkbox"
                  checked={visibleSelected.includes(c.key)}
                  disabled={c.conflict || busy || !manage}
                  onChange={(e) =>
                    setSelected(
                      e.target.checked
                        ? [...selected, c.key]
                        : selected.filter((k) => k !== c.key),
                    )
                  }
                />
                <strong>{changeTitle(c, locale, mainLocale)}</strong>
              </label>
              {c.conflict && <Notice tone="danger">{w("conflict")}</Notice>}
              <details>
                <summary>{w("details")}</summary>
                <code>{c.key}</code>
                <div className="diff-columns">
                  <div>
                    <b>{w("before")}</b>
                    <pre>{JSON.stringify(c.before, null, 2)}</pre>
                  </div>
                  <div>
                    <b>{w("after")}</b>
                    <pre>{JSON.stringify(c.after, null, 2)}</pre>
                  </div>
                </div>
              </details>
            </article>
          ))}
          <Button
            tone="primary"
            type="submit"
            disabled={!visibleSelected.length || busy || !manage}
            onClick={() =>
              void run(async () => {
                await request(`/api/environments/${id}/release`, {
                  approve: true,
                  selections: visibleChanges
                    .filter((c) => visibleSelected.includes(c.key))
                    .map((c) => ({ key: c.key, digest: c.digest })),
                });
                await load();
                if (currentScope.current === scope) setNotice(true);
              })
            }
          >
            {w("release")} · {visibleSelected.length}
          </Button>
        </Card>
      )}
      {notice && <p role="status">{w("released")}</p>}
      {error && <Notice tone="danger">{error}</Notice>}
      <Card>
        <h2>{w("history")}</h2>
        {history.map((r) => (
          <p key={r.id}>
            {date(r.createdAt)} · {r.selections.join(", ")}
          </p>
        ))}
      </Card>
    </div>
  );
}

function changeTitle(change: Change, language: string, mainLocale: string) {
  if (!change.key.startsWith("product:")) return change.key;
  const value = change.after ?? change.before;
  if (!value || typeof value !== "object" || !("name" in value))
    return change.key;
  if (typeof value.name === "string") return value.name || change.key;
  if (!value.name || typeof value.name !== "object") return change.key;
  const names = Object.fromEntries(
    Object.entries(value.name).filter(([, name]) => typeof name === "string"),
  );
  return contentText(names, language, mainLocale) || change.key;
}
