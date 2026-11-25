/** Uses the existing tenant-owned frontend registry, not a parallel domain or Experience store. */
import type { ApiPath } from "../../../../../frontend/src/kit/api";
import {
  useInvalidateRead,
  useRead,
} from "../../../../../frontend/src/admin/shell/useRead";
import {
  Button,
  Field,
  LinkButton,
  Notice,
  Select,
  TextInput,
} from "../../../../../frontend/src/kit/ui";
import { useState } from "react";
import type { RequestFn } from "../../../../../frontend/src/admin/shell/studio-types";
import {
  connectionUrl,
  type FrontendConnection,
} from "../../../../../extensions/apps/storyfront/studio/storyfront-model";
import type { Channel } from "./channel-model";
import { useConnectionText } from "./connection-i18n";
import { useLocale } from "../../../../../frontend/src/shared/i18n/i18n";
import { contentText } from "../../../../../frontend/src/shared/i18n/content-language";
import ConfirmDialog from "../../../../../frontend/src/shared/ui/ConfirmDialog";
export default function ChannelConnections({
  request,
  channel,
  disabled,
  mainLocale = "en-GB",
}: {
  request: RequestFn;
  channel: string;
  disabled: boolean;
  mainLocale?: string;
}) {
  const t = useConnectionText(),
    { locale } = useLocale();
  const frontends = useRead(request, "/api/settings/frontends");
  const automation = useRead(request, "/api/automation");
  const invalidate = useInvalidateRead(request);
  const rows: FrontendConnection[] = frontends.data?.frontends ?? [];
  const channels: Channel[] = automation.data?.channels ?? [];
  const loaded = !!frontends.data && !!automation.data;
  const [actionError, setError] = useState(""),
    [busy, setBusy] = useState(false),
    [notice, setNotice] = useState(false),
    [remove, setRemove] = useState<FrontendConnection | null>(null),
    [alias, setAlias] = useState(""),
    [experience, setExperience] = useState("");
  const error =
    actionError || (frontends.error || automation.error ? t("error") : "");
  const refresh = async () => {
    invalidate("/api/settings/frontends");
    invalidate("/api/automation");
  };
  const save = async (
    data: object,
    method = "PUT",
    path: ApiPath = "/api/settings/frontends",
  ) => {
    setBusy(true);
    setError("");
    setNotice(false);
    try {
      await request(path, data, method);
      await refresh();
      setNotice(true);
      setAlias("");
    } catch {
      setError(t("error"));
    } finally {
      setBusy(false);
    }
  };
  const experiences = [
    ...new Set(rows.map((f) => f.experienceAlias ?? f.alias)),
  ];
  return (
    <section className="channel-connections">
      <header>
        <h3>{t("title")}</h3>
        <p>{t("hint")}</p>
      </header>
      {!loaded && !error && <p role="status">{t("loading")}</p>}
      {loaded && !rows.some((f) => f.channel === channel) && (
        <p>{t("empty")}</p>
      )}
      {rows
        .filter((f) => f.channel === channel)
        .map((f) => (
          <article className="channel-domain-card" key={f.alias}>
            <div>
              <strong>
                {connectionUrl(f.url) ? new URL(f.url).hostname : f.alias}
              </strong>
              <p>
                {t("experience")}: {f.experienceAlias ?? f.alias}
              </p>
            </div>
            <div className="workbench-row">
              {connectionUrl(f.editorUrl, locale) && (
                <LinkButton
                  tone="primary"
                  href={connectionUrl(f.editorUrl, locale)}
                  target="_blank"
                  rel="noopener noreferrer"
                >
                  {t("edit")} ↗
                </LinkButton>
              )}
              {connectionUrl(f.url) && (
                <LinkButton
                  href={connectionUrl(f.url)}
                  target="_blank"
                  rel="noopener noreferrer"
                >
                  {t("open")} ↗
                </LinkButton>
              )}
            </div>
            <Field label={t("channel")}>
              <Select
                value={f.channel}
                disabled={disabled || busy}
                onChange={(e) =>
                  void save({
                    alias: f.alias,
                    experienceAlias: f.experienceAlias ?? f.alias,
                    channel: e.target.value,
                    revision: f.revision,
                  })
                }
              >
                {channels.map((c) => (
                  <option key={c.id} value={c.id}>
                    {contentText(c.data.name, locale, mainLocale) || c.id}
                  </option>
                ))}
              </Select>
            </Field>
            <Button
              type="submit"
              disabled={disabled || busy}
              onClick={() => setRemove(f)}
            >
              {t("disconnect")}
            </Button>
          </article>
        ))}
      {experiences.length > 0 && (
        <form
          className="channel-domain-form"
          onSubmit={(e) => {
            e.preventDefault();
            void save({
              alias,
              experienceAlias: experience || experiences[0],
              channel,
              revision: 0,
            });
          }}
        >
          <h3>{t("add")}</h3>
          <Field label={t("domain")}>
            <TextInput
              value={alias}
              onChange={(e) => setAlias(e.target.value.toLowerCase())}
              pattern="[a-z0-9][a-z0-9-]{0,46}[a-z0-9]"
              required
              disabled={disabled || busy}
            />
          </Field>
          <Field label={t("experience")}>
            <Select
              value={experience || experiences[0]}
              onChange={(e) => setExperience(e.target.value)}
              disabled={disabled || busy}
            >
              {experiences.map((id) => (
                <option key={id}>{id}</option>
              ))}
            </Select>
          </Field>
          <Button
            tone="primary"
            type="submit"
            disabled={disabled || busy || !alias}
          >
            {t("save")}
          </Button>
        </form>
      )}
      <p className="channel-status-hint">{t("hosting")}</p>
      {notice && <p role="status">{t("saved")}</p>}
      {error && <Notice tone="danger">{error}</Notice>}
      {remove && (
        <ConfirmDialog
          title={t("disconnect")}
          confirmLabel={t("disconnect")}
          disabled={busy}
          onCancel={() => setRemove(null)}
          onConfirm={() => {
            void save(
              { revision: remove.revision },
              "DELETE",
              `/api/settings/frontends/${encodeURIComponent(remove.alias)}`,
            );
            setRemove(null);
          }}
        >
          <p>{t("disconnectHint")}</p>
          <strong>{remove.alias}</strong>
        </ConfirmDialog>
      )}
    </section>
  );
}
