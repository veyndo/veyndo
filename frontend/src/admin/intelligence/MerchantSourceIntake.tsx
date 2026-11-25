/** Conversation intake reuses the real private versioned source editor and existing upload API. */
import { useRead } from "../shell/useRead";
import { Button, Notice } from "../../kit/ui";
import { useEffect, useRef } from "react";
import type { RequestFn } from "../shell/studio-types";
import type { Workspace } from "./knowledge-types";
import SourceEditor from "./SourceEditor";
import { useAgentWorkspaceText } from "../../shared/i18n/agent-workspace-i18n";
export default function MerchantSourceIntake({
  request,
  guidance,
  onClose,
}: {
  request: RequestFn;
  guidance: boolean;
  onClose: () => void;
}) {
  const dialog = useRef<HTMLDialogElement>(null),
    t = useAgentWorkspaceText();
  const workspaceRead = useRead<Workspace>(request, "/api/knowledge/workspace");
  const workspace = workspaceRead.data;
  const error = (workspaceRead.error as Error | undefined)?.message ?? "";
  useEffect(() => {
    dialog.current?.showModal();
  }, []);
  return (
    <dialog
      ref={dialog}
      className="studio app-agent-drawer"
      onCancel={onClose}
      aria-label={t(guidance ? "guidance" : "source")}
    >
      <header>
        <h2>{t(guidance ? "guidance" : "source")}</h2>
        <Button type="submit" onClick={onClose}>
          {t("close")}
        </Button>
      </header>
      {guidance && <p>{t("guidanceHint")}</p>}
      {error && <Notice tone="danger">{error}</Notice>}
      {workspace && (
        <SourceEditor
          workspace={workspace}
          request={request}
          initialKind={guidance ? "guidance" : "document"}
          onSaved={onClose}
          onCancel={onClose}
        />
      )}
    </dialog>
  );
}
