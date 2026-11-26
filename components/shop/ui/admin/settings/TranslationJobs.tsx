/** Start catalogue translations, poll durable progress, review paginated drafts and apply bounded revision-checked batches. */
import {
  Badge,
  Button,
  Field,
  Notice,
  Select,
} from "../../../../../frontend/src/kit/ui";
import { useControlText } from "../../../../../frontend/src/shared/i18n/control-i18n";
import { responseError } from "../../../../../frontend/src/shared/i18n/errors-i18n";
import { useEffect, useRef, useState } from "react";
import {
  useInternationalText,
  type InternationalWord,
} from "../../../../../frontend/src/shared/i18n/international-i18n";
import type { InternationalConfig } from "./CommerceSettings";
import type { RequestFn } from "../../../../../frontend/src/admin/shell/studio-types";
type Job = {
  id: string;
  run_task?: string | null;
  target_locale: string;
  status: string;
  processed: number;
  total: number;
  error?: string;
};
type Draft = {
  product_id: string;
  revision: number;
  status: string;
  source: any;
  result: any;
};
export default function TranslationJobs({
  request,
  config,
  disabled,
}: {
  request: RequestFn;
  config: InternationalConfig;
  disabled: boolean;
}) {
  const c = useControlText();
  const { i } = useInternationalText();
  const current = useRef(request);
  current.current = request;
  const [jobs, setJobs] = useState<Job[]>([]),
    [providers, setProviders] = useState<any[]>([]),
    [provider, setProvider] = useState("platform"),
    [target, setTarget] = useState(
      config.locales.find((l) => l !== config.mainLocale) ?? "",
    ),
    [overwrite, setOverwrite] = useState(false),
    [selected, setSelected] = useState(
      new URLSearchParams(location.search).get("translationJob") ?? "",
    ),
    [drafts, setDrafts] = useState<Draft[]>([]),
    [counts, setCounts] = useState<Record<string, number>>({}),
    [cursor, setCursor] = useState<string | null>(null),
    [error, setError] = useState(""),
    [busy, setBusy] = useState(false);
  const active = useRef(true);
  useEffect(() => {
    active.current = true;
    return () => {
      active.current = false;
    };
  }, []);
  const expanded = useRef(false);
  const refresh = async (id = selected, force = false) => {
    const list = await current.current("/api/merchant/translations");
    if (active.current) setJobs(list.jobs);
    if (id && (!expanded.current || force)) {
      const detail = await current.current(`/api/merchant/translations/${id}`);
      if (active.current) {
        setDrafts(detail.items);
        setCounts(detail.counts);
        setCursor(detail.nextCursor);
      }
    }
  };
  const refreshRef = useRef(refresh);
  refreshRef.current = refresh;
  useEffect(() => {
    void refreshRef.current().catch((e) => setError(e.message));
    current
      .current("/api/agent/providers")
      .then((v) => {
        if (active.current) setProviders(v.providers ?? []);
      })
      .catch((e) => setError(e.message));
    const timer = setInterval(
      () =>
        void refreshRef.current().catch((e) => {
          if (active.current) setError(e.message);
        }),
      2500,
    );
    return () => clearInterval(timer);
  }, []);
  const act = async (fn: () => Promise<unknown>, reload = true) => {
    if (busy) return;
    setBusy(true);
    setError("");
    try {
      await fn();
      if (reload) {
        expanded.current = false;
        await refreshRef.current(selected, true);
      }
    } catch (e) {
      if (active.current) setError((e as Error).message);
    } finally {
      if (active.current) setBusy(false);
    }
  };
  const job = jobs.find((j) => j.id === selected);
  return (
    <div className="intl-translation-jobs">
      <div className="intl-section-heading">
        <h3>{i("aiTranslation")}</h3>
        <Badge>AI</Badge>
      </div>
      <p className="intl-hint">{i("aiHint")}</p>
      <fieldset disabled={disabled || busy} className="intl-rule-grid">
        <Field label={i("targetLanguage")}>
          <Select
            value={
              config.locales.includes(target) && target !== config.mainLocale
                ? target
                : ""
            }
            onChange={(e) => setTarget(e.target.value)}
          >
            <option value="">—</option>
            {config.locales
              .filter((l) => l !== config.mainLocale)
              .map((l) => (
                <option key={l} value={l}>
                  {l}
                </option>
              ))}
          </Select>
        </Field>
        <Field label={i("provider")}>
          <Select
            value={provider}
            onChange={(e) => setProvider(e.target.value)}
          >
            {providers.map((p) => (
              <option key={p.id} value={p.id} disabled={!p.configured}>
                {p.id === "platform" ? c("platformDefault") : p.name} ·{" "}
                {p.model}
              </option>
            ))}
          </Select>
        </Field>
        <label className="checkbox-label">
          <input
            type="checkbox"
            checked={overwrite}
            onChange={(e) => setOverwrite(e.target.checked)}
          />
          {i("overwrite")}
        </label>
        <Button
          tone="primary"
          type="button"
          disabled={
            !target ||
            target === config.mainLocale ||
            !providers.some((p) => p.id === provider && p.configured)
          }
          onClick={() =>
            void act(async () => {
              const v = await request("/api/merchant/translations", {
                targetLocale: target,
                inference: { provider, model: null },
                overwrite,
              });
              expanded.current = false;
              setSelected(v.id);
              await refresh(v.id);
            })
          }
        >
          {i("startTranslation")}
        </Button>
      </fieldset>
      {error && <Notice tone="danger">{error}</Notice>}
      <div className="intl-rule-list">
        {jobs.map((j) => (
          <button
            type="button"
            key={j.id}
            aria-pressed={j.id === selected}
            onClick={() => {
              expanded.current = false;
              setSelected(j.id);
              void act(() => refresh(j.id));
            }}
          >
            <span>
              <strong>
                {j.target_locale} · {i(j.status as InternationalWord)}
              </strong>
              <small>
                {i("progress")}: {j.processed} / {j.total}
              </small>
            </span>
            <progress
              aria-label={i("progress")}
              value={j.processed}
              max={Math.max(j.total, 1)}
            />
          </button>
        ))}
      </div>
      {job && (
        <div className="intl-job-detail">
          <div className="intl-section-heading">
            <h3>
              {i("drafts")} · {job.target_locale}
            </h3>
            <span>
              {Object.entries(counts)
                .map(([k, n]) => `${i(k as InternationalWord)}: ${n}`)
                .join(" · ")}
            </span>
          </div>
          {job.error && (
            <Notice tone="danger">
              {responseError(job.error, 400).message}
            </Notice>
          )}
          {job.run_task && (
            <p className="intl-hint">{i("managedTranslation")}</p>
          )}
          <div className="intl-inline">
            {!job.run_task && job.status === "failed" && (
              <Button
                type="button"
                disabled={disabled || busy}
                onClick={() =>
                  void act(() =>
                    request(
                      `/api/merchant/translations/${job.id}`,
                      { action: "resume" },
                      "PUT",
                    ),
                  )
                }
              >
                {i("resume")}
              </Button>
            )}
            {!job.run_task &&
              ["queued", "processing", "failed"].includes(job.status) && (
                <Button
                  type="button"
                  disabled={disabled || busy}
                  onClick={() =>
                    void act(() =>
                      request(
                        `/api/merchant/translations/${job.id}`,
                        { action: "cancel" },
                        "PUT",
                      ),
                    )
                  }
                >
                  {i("cancel")}
                </Button>
              )}
            {job.status === "ready" && counts.ready > 0 && (
              <Button
                tone="primary"
                type="button"
                disabled={disabled || busy}
                onClick={() =>
                  void act(async () => {
                    let remaining = 1;
                    while (remaining && active.current) {
                      const result = await request(
                        `/api/merchant/translations/${job.id}/apply`,
                        {},
                      );
                      remaining = result.remaining;
                    }
                  })
                }
              >
                {i("applyAll")}
              </Button>
            )}
          </div>
          {drafts.map((d) => (
            <details key={d.product_id} className="intl-draft">
              <summary>
                <strong>{d.result.name ?? d.source.name}</strong>
                <span>
                  {i(d.status as InternationalWord)} · #{d.revision}
                </span>
              </summary>
              <div className="intl-draft-comparison">
                <div>
                  <h4>{i("mainLanguage")}</h4>
                  <p>{d.source.description}</p>
                </div>
                <div>
                  <h4>{job.target_locale}</h4>
                  <p>{d.result.description ?? "—"}</p>
                </div>
              </div>
              {d.status === "ready" && job.status === "ready" && (
                <Button
                  type="button"
                  disabled={disabled || busy}
                  onClick={() =>
                    void act(() =>
                      request(`/api/merchant/translations/${job.id}/apply`, {
                        productId: d.product_id,
                      }),
                    )
                  }
                >
                  {i("applyDraft")}
                </Button>
              )}
            </details>
          ))}
          {cursor && (
            <Button
              type="button"
              onClick={() =>
                void act(async () => {
                  const v = await request(
                    `/api/merchant/translations/${job.id}?cursor=${encodeURIComponent(cursor)}`,
                  );
                  expanded.current = true;
                  setDrafts((d) => [...d, ...v.items]);
                  setCursor(v.nextCursor);
                }, false)
              }
            >
              {i("more")}
            </Button>
          )}
        </div>
      )}
    </div>
  );
}
