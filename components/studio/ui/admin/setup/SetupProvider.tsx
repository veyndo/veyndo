/** Personal, revisioned setup progress reuses the existing workspace store and session fences. */
import {
  useRead,
  useSetRead,
} from "../../../../../frontend/src/admin/shell/useRead";
import {
  createContext,
  useCallback,
  useContext,
  useEffect,
  useRef,
  useState,
  type ReactNode,
} from "react";
import { useStudio } from "../../../../../frontend/src/admin/shell/StudioContext";
import type { WorkspaceRecord } from "../workspace/workspace-model";
import { withSetup, type SetupProgress } from "./setup-model";
import { useSetupText } from "./setup-i18n";
function useSetupController() {
  const studio = useStudio(),
    t = useSetupText();
  const { connected, environment, access, liveRequest, workspace, auth } =
    studio;
  const eligible =
    access.includes("settings.read") && access.includes("settings.write");
  const available = eligible && connected && !environment && !!auth.identity;
  const scope = JSON.stringify([workspace, auth.identity?.user.id]);
  const [busy, setBusy] = useState(false);
  const [saveError, setError] = useState("");
  const recordRead = useRead<WorkspaceRecord>(
    liveRequest,
    available ? "/api/studio/workspace" : null,
  );
  const setRead = useSetRead(liveRequest);
  const loading =
    available &&
    (recordRead.status === "idle" || recordRead.status === "loading");
  const error =
    saveError ||
    (available && recordRead.status === "error" ? t("loadError") : "");
  const fence = useRef(0),
    saving = useRef(false);
  const current = useRef({ available, scope, liveRequest });
  current.current = { available, scope, liveRequest };
  const record = available ? recordRead.data : undefined;
  useEffect(() => {
    ++fence.current;
    setError("");
    return () => {
      fence.current++;
    };
  }, [available, liveRequest, scope]);
  const save = useCallback(
    async (setup: SetupProgress) => {
      if (!available || saving.current) return false;
      const generation = fence.current;
      const stillCurrent = () =>
        generation === fence.current &&
        current.current.available &&
        current.current.scope === scope;
      saving.current = true;
      setBusy(true);
      setError("");
      try {
        // Merge only this preference into the latest layout; CAS still rejects a simultaneous writer.
        const latest = (await liveRequest(
          "/api/studio/workspace",
        )) as WorkspaceRecord;
        if (!stillCurrent()) return false;
        // A fresh layout may be merged, but another tab's setup review must never be replaced.
        if (
          JSON.stringify(latest.data.setup) !==
          JSON.stringify(record?.data.setup)
        )
          throw new Error("Setup changed in another session");
        const next = (await liveRequest(
          "/api/studio/workspace",
          {
            revision: latest.revision,
            data: withSetup(latest.data, setup),
          },
          "PUT",
        )) as WorkspaceRecord;
        if (!stillCurrent()) return false;
        setRead("/api/studio/workspace", next, undefined);
        return true;
      } catch {
        if (stillCurrent()) setError(t("saveError"));
        return false;
      } finally {
        saving.current = false;
        if (stillCurrent()) setBusy(false);
      }
    },
    [available, liveRequest, scope, record, t],
  );
  return {
    available,
    eligible,
    record,
    progress: record?.data.setup,
    busy,
    loading,
    error,
    reload: () => {
      setError("");
      void recordRead.refetch();
    },
    save,
    autoOpen:
      available &&
      record?.revision === 0 &&
      !record.data.setup &&
      studio.data?.summary.orders === 0,
  };
}
type SetupController = ReturnType<typeof useSetupController>;
const SetupContext = createContext<SetupController | null>(null);
export function SetupProvider({ children }: { children: ReactNode }) {
  const value = useSetupController();
  return (
    <SetupContext.Provider value={value}>{children}</SetupContext.Provider>
  );
}
export function useShopSetup() {
  const value = useContext(SetupContext);
  if (!value) throw new Error("SetupProvider required");
  return value;
}
