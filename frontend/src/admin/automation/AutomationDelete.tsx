/** One confirmation and a server recheck; stale versions and used definitions never disappear silently. */
import { Button, Notice } from "../../kit/ui";
import { useState } from "react";
import type { RequestFn } from "../shell/studio-types";
import type { Kind } from "./automation-types";
import ConfirmDialog from "../../shared/ui/ConfirmDialog";
import { contentText } from "../../shared/i18n/content-language";
import { useLocale } from "../../shared/i18n/i18n";
import { useLifecycleText, type DependencyKind } from "./lifecycle-i18n";
export default function AutomationDelete({
  request,
  kind,
  id,
  revision,
  disabled,
  onDeleted,
  onReference,
}: {
  request: RequestFn;
  kind: Kind;
  id: string;
  revision: number;
  disabled: boolean;
  onDeleted: () => void | Promise<void>;
  onReference?: (kind: Kind, id: string) => void;
}) {
  const t = useLifecycleText(),
    { locale } = useLocale();
  const [dependencies, setDependencies] = useState<
    | {
        kind: DependencyKind;
        id: string;
        count: number;
        name?: Record<string, string>;
      }[]
    | null
  >(null);
  const [busy, setBusy] = useState(false),
    [error, setError] = useState("");
  if (!revision || (kind === "channels" && id === "default")) return null;
  const path = `/api/automation/${kind}/${encodeURIComponent(id)}`;
  const inspect = async () => {
    setBusy(true);
    setError("");
    try {
      const v = await request(`${path}/dependencies`);
      setDependencies(v.dependencies);
    } catch (e) {
      setError((e as Error).message);
    } finally {
      setBusy(false);
    }
  };
  const remove = async () => {
    setBusy(true);
    setError("");
    try {
      await request(path, { revision }, "DELETE");
      setDependencies(null);
      await onDeleted();
    } catch (e) {
      setError((e as Error).message);
      const v = await request(`${path}/dependencies`).catch(() => null);
      if (v) setDependencies(v.dependencies);
    } finally {
      setBusy(false);
    }
  };
  return (
    <div className="automation-delete">
      <Button
        type="button"
        disabled={disabled || busy}
        onClick={() => void inspect()}
      >
        {busy ? t("checking") : t("delete")}
      </Button>
      {dependencies !== null && (
        <ConfirmDialog
          title={t("confirm")}
          confirmLabel={t("delete")}
          disabled={busy || dependencies.length > 0}
          onCancel={() => {
            if (!busy) {
              setDependencies(null);
              setError("");
            }
          }}
          onConfirm={() => void remove()}
        >
          <p>{t(dependencies.length ? "blocked" : "explanation")}</p>
          <ul>
            {dependencies.map((d, i) => (
              <li key={`${d.kind}:${d.id}:${i}`}>
                {onReference &&
                ["rules", "flows", "promotions"].includes(d.kind) ? (
                  <Button
                    type="button"
                    onClick={() => {
                      setDependencies(null);
                      onReference(d.kind as Kind, d.id);
                    }}
                  >
                    {contentText(d.name ?? {}, locale, "en-GB") || d.id} ↗
                  </Button>
                ) : (
                  <span>
                    {t(d.kind)} · {d.count}
                  </span>
                )}
              </li>
            ))}
          </ul>
          {error && <Notice tone="danger">{error}</Notice>}
        </ConfirmDialog>
      )}
      {error && dependencies === null && <Notice tone="danger">{error}</Notice>}
    </div>
  );
}
