/** Shared confirmation on the kit dialog: localized cancel and confirm actions, Escape cancels, the confirm action can be disabled. */
import type { ReactNode } from "react";
import { Button, Dialog } from "../../kit/ui";
import { useWorkspaceText } from "../i18n/workspace-i18n";
import "./confirm-dialog.css";
export default function ConfirmDialog({
  title,
  children,
  onCancel,
  onConfirm,
  confirmLabel,
  disabled = false,
}: {
  title: string;
  children: ReactNode;
  onCancel: () => void;
  onConfirm: () => void;
  confirmLabel?: string;
  disabled?: boolean;
}) {
  const { w } = useWorkspaceText();
  return (
    <Dialog
      title={title}
      onClose={onCancel}
      footer={
        <>
          <Button onClick={onCancel}>{w("cancel")}</Button>
          <Button tone="primary" disabled={disabled} onClick={onConfirm}>
            {confirmLabel ?? w("confirm")}
          </Button>
        </>
      }
    >
      <div className="confirm-content">{children}</div>
    </Dialog>
  );
}
