/** Lifecycle actions use shared dependency deletion and one-use preview admission. */
import { Button, Notice } from "../../kit/ui";
import { useState } from "react";
import AutomationDelete from "../automation/AutomationDelete";
import type { RequestFn } from "../shell/studio-types";
import { posUrl, channelUrl, type Channel } from "./channel-model";
import { useWorkspaceText } from "../workspace/workspace-i18n";
import { useChannelText } from "./channel-i18n";
import { connectionUrl } from "../../../../extensions/apps/storyfront/studio/storyfront-model";
export default function ChannelActions({
  draft,
  request,
  workspace,
  disabled,
  onToggle,
  onDeleted,
  onMarketplace,
  onDesign,
}: {
  draft: Channel;
  request: RequestFn;
  workspace: string;
  disabled: boolean;
  onToggle: () => void;
  onDeleted: () => void;
  onMarketplace?: () => void;
  onDesign?: () => void;
}) {
  const personal = useWorkspaceText();
  const t = useChannelText(),
    [busy, setBusy] = useState(false),
    [error, setError] = useState("");
  const preview = async () => {
    const popup = window.open("about:blank", "_blank");
    setBusy(true);
    setError("");
    try {
      const mounts = await request("/api/settings/frontends");
      const alias = mounts.frontends?.find(
        (f: { channel: string }) => f.channel === draft.id,
      )?.alias;
      const value = await request(
        `/api/automation/channels/${encodeURIComponent(draft.id)}/preview`,
        alias ? { alias } : {},
      );
      const url = connectionUrl(value.url);
      if (!url || !popup) throw new Error();
      popup.opener = null;
      popup.location.replace(url);
    } catch {
      popup?.close();
      setError(t("previewError"));
    } finally {
      setBusy(false);
    }
  };
  return (
    <>
      {draft.data.kind === "storefront" && onDesign && (
        <Button type="button" onClick={onDesign}>
          {personal("design")} ↗
        </Button>
      )}
      {draft.data.kind === "marketplace" && (
        <Button
          type="button"
          disabled={disabled || busy}
          onClick={() =>
            onMarketplace
              ? onMarketplace()
              : location.assign(
                  `/?shop=${encodeURIComponent(workspace)}&studio=apps&app=marketplaces&channel=${encodeURIComponent(draft.id)}#merchant`,
                )
          }
        >
          {t("marketplaceManage")}
        </Button>
      )}
      {draft.data.kind === "pos" && draft.data.active && (
        <a className="studio-secondary" href={posUrl(workspace, draft.id)}>
          {t("pos")} ↗
        </a>
      )}
      {!["pos", "marketplace"].includes(draft.data.kind) &&
        draft.data.active &&
        draft.data.visibility !== "private" && (
          <a
            className="studio-secondary"
            href={channelUrl(workspace, draft.id)}
            target="_blank"
            rel="noopener noreferrer"
          >
            {t("preview")} ↗
          </a>
        )}
      {!["pos", "marketplace"].includes(draft.data.kind) && (
        <Button
          type="submit"
          disabled={disabled || busy}
          onClick={() => void preview()}
        >
          {t("previewPrivate")} ↗
        </Button>
      )}
      <Button type="submit" disabled={disabled || busy} onClick={onToggle}>
        {t(draft.data.active ? "deactivate" : "activate")}
      </Button>
      <AutomationDelete
        request={request}
        kind="channels"
        id={draft.id}
        revision={draft.revision}
        disabled={disabled || busy}
        onDeleted={onDeleted}
      />
      {error && <Notice tone="danger">{error}</Notice>}
    </>
  );
}
