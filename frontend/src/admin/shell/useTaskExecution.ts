/** Observe the original task worker; an accepted command is not a completed write. */
import { useEffect, useRef, useState } from "react";
import type { RequestFn } from "./studio-types";
import { useRead } from "./useRead";
import type { SavedRun } from "./useChatTurns";
type Scope = { id: string; request: RequestFn };
type Progress = {
  scope: Scope;
  busy: boolean;
  accepted: boolean;
  applied: boolean;
  run?: SavedRun;
  error: string;
  refresh: number;
};
export function useTaskExecution(
  id: string,
  applied: boolean,
  request: RequestFn,
  executionStarted = false,
) {
  const scope = useRef<Scope>({ id, request });
  if (scope.current.id !== id || scope.current.request !== request)
    scope.current = { id, request };
  const owner = scope.current;
  const initial = (): Progress => ({
    scope: owner,
    busy: false,
    accepted: executionStarted,
    applied,
    error: "",
    refresh: 0,
  });
  const [saved, setSaved] = useState<Progress>(initial);
  const state = saved.scope === owner ? saved : initial();
  const accepted = state.accepted || executionStarted;
  const update = (patch: Partial<Progress>) => {
    if (scope.current === owner)
      setSaved((old) => ({
        ...(old.scope === owner ? old : initial()),
        ...patch,
      }));
  };
  // An accepted command is followed until its run settles (every 1.5 seconds).
  const [following, setFollowing] = useState(true);
  const runRead = useRead<SavedRun>(
    request,
    accepted ? `/api/agent/runs/${encodeURIComponent(id)}` : null,
    { refetchMs: following ? 1500 : undefined },
  );
  useEffect(() => {
    const run = runRead.data;
    if (!run) return;
    update({
      run,
      applied: run.state === "ready" && run.approvalRequired === false,
      error: "",
    });
    setFollowing(run.state === "queued" || run.state === "running");
  }, [runRead.data]);
  useEffect(() => {
    if (runRead.error) update({ error: (runRead.error as Error).message });
  }, [runRead.error]);
  useEffect(() => {
    if (state.refresh && accepted) {
      setFollowing(true);
      void runRead.refetch();
    }
  }, [state.refresh]);
  const approve = async () => {
    if (state.busy || accepted || state.applied || applied) return;
    update({ busy: true, error: "" });
    try {
      const result = await request(
        `/api/agent/tasks/${encodeURIComponent(id)}/apply`,
        { approve: true },
      );
      update({
        applied: result.applied === true,
        accepted: result.queued === true,
      });
    } catch (e) {
      update({ error: (e as Error).message });
    } finally {
      update({ busy: false });
    }
  };
  const control = async (action: "resume" | "cancel") => {
    if (!state.run || state.busy) return;
    update({ busy: true, error: "" });
    try {
      const run: SavedRun = await request(
        `/api/agent/runs/${encodeURIComponent(id)}/${action}`,
        { revision: state.run.revision, approve: true },
      );
      update({ run, refresh: state.refresh + 1 });
    } catch (e) {
      update({ error: (e as Error).message });
    } finally {
      update({ busy: false });
    }
  };
  return {
    ...state,
    accepted,
    applied: state.applied || applied,
    approve,
    control,
    reload: () => update({ refresh: state.refresh + 1 }),
  };
}
