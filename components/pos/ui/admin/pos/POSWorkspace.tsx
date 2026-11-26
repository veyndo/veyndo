/** Native Studio register; all reads and writes use authenticated, tenant-scoped shared commerce operations. */
import { useRead } from "../../../../../frontend/src/admin/shell/useRead";
import { Button, Field, Select } from "../../../../../frontend/src/kit/ui";
import { useEffect, useState } from "react";
import type { ApiRequestFn as RequestFn } from "../../../../../frontend/src/shared/api/types";
import type { Channel } from "../../../../marketing/ui/admin/channels/channel-model";
import { useLocale } from "../../../../../frontend/src/shared/i18n/i18n";
import { contentText } from "../../../../../frontend/src/shared/i18n/content-language";
import Icon from "../../../../../frontend/src/shared/ui/Icon";
import { usePosText } from "./pos-i18n";
import Register from "./Register";
import { useCashierFocus } from "./useCashierFocus";
import "./pos.css";
import "./pos-receipt.css";
import "./pos-responsive.css";
import "./cashier.css";
export type POSProps = {
  request: RequestFn;
  workspace: string;
  onChannels: () => void;
  onOrder: (id: string) => void;
  download: (path: string) => Promise<void>;
};
export default function POSWorkspace(props: POSProps) {
  const t = usePosText();
  const { locale } = useLocale();
  const focus = useCashierFocus();
  const [selected, setSelected] = useState(
    new URLSearchParams(location.search).get("posChannel") ?? "",
  );
  const posChannels = useRead(props.request, "/api/pos/channels");
  const access = useRead(props.request, "/api/auth/access");
  const permitted =
    !!access.data?.permissions?.includes("pos.use") &&
    !!access.data?.permissions?.includes("orders.write");
  const ready = !!posChannels.data && permitted;
  const rights: string[] = ready ? access.data.permissions : [];
  const channels: Channel[] = ready
    ? posChannels.data.channels.filter((v: Channel) => v.data.kind === "pos")
    : [];
  const main: string = ready ? posChannels.data.mainLocale : "";
  const error =
    (posChannels.error as Error | undefined)?.message ||
    (access.error as Error | undefined)?.message ||
    (access.data && !permitted ? t("permission") : "");
  // A listed channel stays selected; otherwise the first active one is chosen.
  useEffect(() => {
    if (!ready) return;
    setSelected((old) =>
      channels.some((c) => c.id === old)
        ? old
        : (channels.find((c) => c.data.active)?.id ?? ""),
    );
  }, [ready, posChannels.data]);
  const channel = channels.find((c) => c.id === selected && c.data.active);
  return (
    <div className="studio-page pos-workspace" data-focus={focus.active}>
      <header className="pos-heading">
        <div>
          <span className="pos-eyebrow">
            <Icon name="register" />
            {t("online")}
          </span>
          <h1>{t("title")}</h1>
          <p>{t("hint")}</p>
        </div>
        <div className="pos-toolbar">
          <button
            className="pos-focus-toggle"
            onClick={focus.toggle}
            aria-pressed={focus.active}
          >
            <Icon name={focus.active ? "arrow" : "frame"} />
            {t(focus.active ? "exitFocus" : "focus")}
          </button>
          {focus.active && (
            <button aria-label={t("fullscreen")} onClick={focus.fullscreen}>
              <Icon name="frame" />
            </button>
          )}
          <Field label={t("choose")}>
            <Select
              value={selected}
              onChange={(e) => {
                setSelected(e.target.value);
                const url = new URL(location.href);
                url.searchParams.set("posChannel", e.target.value);
                history.replaceState(null, "", url);
              }}
            >
              {channels
                .filter((c) => c.data.active)
                .map((c) => (
                  <option key={c.id} value={c.id}>
                    {contentText(c.data.name, locale, main) || c.id}
                  </option>
                ))}
            </Select>
          </Field>
          <button onClick={props.onChannels}>
            <Icon name="settings" />
            {t("setup")}
          </button>
        </div>
      </header>
      {error && (
        <div className="pos-error" role="alert">
          {error}
        </div>
      )}
      {ready && !channel && (
        <section className="pos-start">
          <div className="pos-start-icon">
            <Icon name="register" size={52} />
          </div>
          <h2>{t("empty")}</h2>
          <Button tone="primary" type="submit" onClick={props.onChannels}>
            {t("setup")}
            <Icon name="arrow" />
          </Button>
        </section>
      )}
      {channel && (
        <Register
          key={channel.id}
          {...props}
          channel={channel}
          mainLocale={main}
          rights={rights}
        />
      )}
    </div>
  );
}
