/** Personal current-rights stop control; operational revisions cannot be published from a sandbox. */
import {
  useRead,
  useSetRead,
} from "../../../../../../frontend/src/admin/shell/useRead";
import { Button, Card, Notice } from "../../../../../../frontend/src/kit/ui";
import { useEffect, useRef, useState } from "react";
import type { RequestFn } from "../../../../../../frontend/src/admin/shell/studio-types";
import ConfirmDialog from "../../../../../../frontend/src/shared/ui/ConfirmDialog";
import { useAgentControlText } from "../../../../../identity/ui/admin/team/agent-control-i18n";
type Control = {
  stopped: boolean;
  revision: number;
  liveShop: string;
  owner: RequestFn;
};
export default function AgentControls({ request }: { request: RequestFn }) {
  const t = useAgentControlText();
  const current = useRef(request);
  current.current = request;
  const [confirm, setConfirm] = useState(false),
    [busy, setBusy] = useState(false),
    [actionError, setError] = useState("");
  const access = useRead(request, "/api/auth/access");
  const allowed =
    access.data?.personalSession === true &&
    !!access.data?.permissions?.includes("ai.manage");
  const control = useRead(request, allowed ? "/api/agent/control" : null);
  const setRead = useSetRead(request);
  const state: Control | null = control.data
    ? { ...control.data, owner: request }
    : null;
  const error =
    actionError ||
    (access.error as Error | undefined)?.message ||
    (control.error as Error | undefined)?.message ||
    "";
  useEffect(() => {
    setConfirm(false);
    setBusy(false);
    setError("");
  }, [request]);
  if (state?.owner !== request && !error) return null;
  return (
    <Card className="api-key-panel">
      <h2>{t("title")}</h2>
      {state && (
        <>
          <strong>{t(state.stopped ? "stopped" : "running")}</strong>
          <p>{t("stopHint")}</p>
          <Button
            type="submit"
            disabled={busy}
            onClick={() => setConfirm(true)}
          >
            {t(state.stopped ? "resume" : "stop")}
          </Button>
        </>
      )}
      {state && confirm && (
        <ConfirmDialog
          title={t(state.stopped ? "resume" : "stop")}
          confirmLabel={t(state.stopped ? "resume" : "stop")}
          disabled={busy}
          onCancel={() => setConfirm(false)}
          onConfirm={async () => {
            setBusy(true);
            setError("");
            try {
              const next = await request(
                "/api/agent/control",
                {
                  stopped: !state.stopped,
                  revision: state.revision,
                  approve: true,
                },
                "POST",
              );
              if (current.current === request) {
                setRead("/api/agent/control", next);
                setConfirm(false);
              }
            } catch (e) {
              if (current.current === request) setError((e as Error).message);
            } finally {
              if (current.current === request) setBusy(false);
            }
          }}
        >
          <p>{t(state.stopped ? "resumeHint" : "stopHint")}</p>
        </ConfirmDialog>
      )}
      {error && <Notice tone="danger">{error}</Notice>}
    </Card>
  );
}
