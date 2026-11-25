/** Recheck current server grants; discard previously loaded views when a live role changes. */
import { useEffect, useRef } from "react";
import type { Dispatch, SetStateAction } from "react";
import type { RequestFn } from "./studio-types";
import { useRead } from "./useRead";
export function useStudioPermissions(
  active: boolean,
  paused: boolean,
  connected: boolean,
  request: RequestFn,
  rights: string[],
  setRights: Dispatch<SetStateAction<string[]>>,
  invalidate: () => void,
) {
  const previous = useRef("");
  const callback = useRef(invalidate);
  callback.current = invalidate;
  useEffect(() => {
    const key = JSON.stringify(rights);
    if (previous.current && previous.current !== key) callback.current();
    previous.current = connected ? key : "";
  }, [rights, connected]);
  // Current grants are read again every 30 seconds and on focus while visible.
  const access = useRead(
    request,
    active && !paused ? "/api/auth/access" : null,
    { refetchMs: 30_000, staleMs: 30_000 },
  );
  useEffect(() => {
    if (!active || paused) return;
    const refresh = () => {
      if (document.visibilityState !== "hidden") void access.refetch();
    };
    window.addEventListener("focus", refresh);
    return () => window.removeEventListener("focus", refresh);
  }, [active, paused, access.refetch]);
  useEffect(() => {
    if (!active) {
      setRights([]);
      callback.current();
      return;
    }
    if (paused) return;
    if (access.status === "error") setRights([]);
    else if (access.data)
      setRights((old) =>
        JSON.stringify(old) === JSON.stringify(access.data.permissions)
          ? old
          : access.data.permissions,
      );
  }, [active, paused, access.data, access.status, setRights]);
}
