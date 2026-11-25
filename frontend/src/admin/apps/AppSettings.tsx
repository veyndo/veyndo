/** Installed settings use the existing encrypted secret owner; secret inputs are write-only and cleared after save. */
import type { ApiPath } from "../../kit/api";
import { Button, Notice } from "../../kit/ui";
import "./app-settings.css";
import { useEffect, useState } from "react";
import type { Manifest } from "../../shared/apps/native/types";
import type { RequestFn } from "../shell/studio-types";
import { useRead, useSetRead } from "../shell/useRead";
import { useAppSettingText } from "../../shared/i18n/app-settings-i18n";
import { useLocale } from "../../shared/i18n/i18n";
import { contentText } from "../../shared/i18n/content-language";
type State = {
  fields: NonNullable<Manifest["settings"]>;
  values: Record<string, string | number | boolean | null>;
  configured: string[];
  revision: number;
  digest: string;
  current: boolean;
  canManage: boolean;
  encryptionReady: boolean;
};
export default function AppSettings({
  app,
  request,
  mainLocale,
}: {
  app: string;
  request: RequestFn;
  mainLocale: string;
}) {
  const t = useAppSettingText(),
    { locale } = useLocale(),
    [values, setValues] = useState<State["values"]>({}),
    [approve, setApprove] = useState(false),
    [actionError, setError] = useState(""),
    [busy, setBusy] = useState(false),
    [saved, setSaved] = useState(false);
  const settingsPath: ApiPath = `/api/apps/${encodeURIComponent(app)}/settings`;
  const settings = useRead<State>(request, settingsPath);
  const setRead = useSetRead(request);
  const state = settings.data ?? null;
  const error =
    actionError || (settings.error as Error | undefined)?.message || "";
  // A newly loaded state replaces the form values; drafts never outlive it.
  useEffect(() => {
    setValues(state?.values ?? {});
    setApprove(false);
  }, [state]);
  useEffect(() => setError(""), [app]);
  const load = async () => setRead(settingsPath, await request(settingsPath));
  return (
    <section className="app-installed-settings">
      <h2>{t("settings")}</h2>
      <p>{t("hint")}</p>
      <p>{t("runtime")}</p>
      {state && (
        <form
          onSubmit={async (e) => {
            e.preventDefault();
            setBusy(true);
            setError("");
            setSaved(false);
            try {
              await request(
                `/api/apps/${encodeURIComponent(app)}/settings`,
                {
                  revision: state.revision,
                  digest: state.digest,
                  approve,
                  values,
                },
                "PUT",
              );
              setValues({});
              await load();
              setSaved(true);
            } catch (e) {
              setError((e as Error).message);
            } finally {
              setBusy(false);
            }
          }}
        >
          <fieldset
            disabled={busy || !state.canManage || !state.encryptionReady}
          >
            {!state.current && <p role="status">{t("stale")}</p>}
            <div className="app-settings-values">
              {state.fields.map((f) => (
                <label key={f.key}>
                  {contentText(f.label, locale, mainLocale) || f.key}
                  {f.kind === "boolean" ? (
                    <input
                      type="checkbox"
                      checked={values[f.key] === true}
                      onChange={(e) =>
                        setValues({ ...values, [f.key]: e.target.checked })
                      }
                    />
                  ) : (
                    <input
                      type={
                        f.kind === "secret"
                          ? "password"
                          : f.kind === "integer"
                            ? "number"
                            : "text"
                      }
                      autoComplete="off"
                      required={f.required && !state.configured.includes(f.key)}
                      placeholder={
                        f.kind === "secret" && state.configured.includes(f.key)
                          ? t("configured")
                          : undefined
                      }
                      value={String(values[f.key] ?? "")}
                      maxLength={4096}
                      onChange={(e) => {
                        const next = { ...values };
                        if (f.kind === "secret" && !e.target.value)
                          delete next[f.key];
                        else
                          next[f.key] =
                            f.kind === "integer"
                              ? Number(e.target.value)
                              : e.target.value;
                        setValues(next);
                      }}
                    />
                  )}
                </label>
              ))}
            </div>
            <label className="app-check">
              <input
                type="checkbox"
                checked={approve}
                onChange={(e) => setApprove(e.target.checked)}
              />
              {t("approve")}
            </label>
            <Button tone="primary" type="submit" disabled={!approve}>
              {t("save")}
            </Button>
          </fieldset>
        </form>
      )}
      {saved && <p role="status">{t("saved")}</p>}
      {error && <Notice tone="danger">{error}</Notice>}
    </section>
  );
}
