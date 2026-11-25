/** Explicit consent for digest-bound server callback keys; plaintext is shown once and never persisted in the browser. */
import { Button, Field, Notice, TextInput } from "../../kit/ui";
import { useEffect, useState } from "react";
import type { RequestFn } from "../shell/studio-types";
import { useRead, useSetRead } from "../shell/useRead";
import ConfirmDialog from "../../shared/ui/ConfirmDialog";
import {
  useAppAccessText,
  type CallbackPermission,
} from "../../shared/i18n/app-access-i18n";
type Access = {
  canManage: boolean;
  digest: string;
  permissions: CallbackPermission[];
  keys: {
    id: string;
    permissions: string[];
    expiresAt: string;
    packageDigest: string;
  }[];
};
export default function AppAccess({
  app,
  request,
}: {
  app: string;
  request: RequestFn;
}) {
  const t = useAppAccessText();
  const path = `/api/apps/${app}/credentials`;
  const access = useRead<Access>(request, path);
  const setRead = useSetRead(request);
  const data = access.data ?? null;
  const [selected, setSelected] = useState<CallbackPermission[]>([]),
    [days, setDays] = useState("30"),
    [secret, setSecret] = useState(""),
    [actionError, setError] = useState(""),
    [busy, setBusy] = useState(false),
    [revoke, setRevoke] = useState<string | null>(null);
  useEffect(() => {
    setSelected([]);
    setSecret("");
    setError("");
  }, [app]);
  const error = actionError || (access.error as Error | undefined)?.message;
  const create = async () => {
    if (!data) return;
    setBusy(true);
    setSecret("");
    setError("");
    try {
      const result = await request(`/api/apps/${app}/credentials`, {
        digest: data.digest,
        approve: true,
        permissions: selected,
        expiresInDays: Number(days),
      });
      setSecret(result.key);
      setRead(path, await request(path));
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
      await request(
        `/api/apps/${app}/credentials/${revoke}`,
        undefined,
        "DELETE",
      );
      setRevoke(null);
      setSecret("");
      setRead(path, await request(path));
    } catch (e) {
      setError((e as Error).message);
    } finally {
      setBusy(false);
    }
  };
  if (data && !data.permissions.length) return null;
  const validPii =
    !selected.includes("customers.pii") ||
    selected.some((p) => p === "orders.read" || p === "customers.read");
  return (
    <section className="app-contract-summary">
      <h3>{t("title")}</h3>
      <p>{t("hint")}</p>
      <form
        onSubmit={(e) => {
          e.preventDefault();
          void create();
        }}
      >
        <fieldset
          disabled={busy || !data?.canManage}
          className="app-access-permissions"
        >
          {data?.permissions.map((p) => (
            <label key={p}>
              <input
                type="checkbox"
                checked={selected.includes(p)}
                onChange={(e) =>
                  setSelected((old) =>
                    e.target.checked ? [...old, p] : old.filter((v) => v !== p),
                  )
                }
              />
              {t(p)}
            </label>
          ))}
          <Field label={t("days")}>
            <TextInput
              type="number"
              required
              min="1"
              max="90"
              value={days}
              onChange={(e) => setDays(e.target.value)}
            />
          </Field>
          <Button
            tone="primary"
            type="submit"
            disabled={
              !data ||
              !selected.length ||
              !validPii ||
              !Number.isInteger(Number(days)) ||
              Number(days) < 1 ||
              Number(days) > 90
            }
          >
            {t("create")}
          </Button>
        </fieldset>
      </form>
      {secret && (
        <div role="status">
          <p>{t("secret")}</p>
          <code className="app-key-secret">{secret}</code>
        </div>
      )}
      {data?.keys.map((key) => (
        <div className="app-access-key" key={key.id}>
          <code>{key.id}</code>
          <time dateTime={key.expiresAt}>
            {new Date(key.expiresAt).toLocaleString()}
          </time>
          {(key.packageDigest !== data.digest ||
            Date.parse(key.expiresAt) <= Date.now()) && (
            <small>{t("expired")}</small>
          )}
          <Button
            type="submit"
            disabled={busy || !data?.canManage}
            onClick={() => setRevoke(key.id)}
          >
            {t("revoke")}
          </Button>
        </div>
      ))}
      {data && !data.keys.length && <p>{t("none")}</p>}
      {error && <Notice tone="danger">{error}</Notice>}
      {revoke && (
        <ConfirmDialog
          title={t("revoke")}
          confirmLabel={t("revoke")}
          disabled={busy}
          onCancel={() => setRevoke(null)}
          onConfirm={() => void remove()}
        >
          <p>{t("revokeHint")}</p>
        </ConfirmDialog>
      )}
    </section>
  );
}
