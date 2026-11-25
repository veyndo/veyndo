/** Actor-private server autosaves serialize writes and preserve optimistic revisions across app switches. */
import { useRead } from "../shell/useRead";
import { useEffect, useRef, useState } from "react";
import type { RequestFn } from "../shell/studio-types";
import type { Manifest } from "../../shared/apps/native/types";
type Draft = {
  id: string;
  manifest: Manifest;
  environment: string | null;
  revision: number;
};
export function useDraftStorage(
  request: RequestFn,
  manifest: Manifest,
  environment: string,
  edited: boolean,
  restore: (m: Manifest, env: string) => void,
) {
  const latest = useRef({ manifest, environment, edited, restore });
  latest.current = { manifest, environment, edited, restore };
  const [ready, setReady] = useState(false),
    [pending, setPending] = useState(false),
    [error, setError] = useState(""),
    [restored, setRestored] = useState(false);
  const generation = useRef(0),
    revisions = useRef(new Map<string, number>()),
    snapshots = useRef(new Map<string, string>()),
    writing = useRef<number | null>(null);
  const stored = useRead<{ drafts: Draft[] }>(request, "/api/developer/drafts");
  useEffect(() => {
    ++generation.current;
    revisions.current.clear();
    snapshots.current.clear();
    setReady(false);
    setRestored(false);
    return () => {
      generation.current++;
    };
  }, [request]);
  // Each answer records revisions and snapshots; the newest draft is restored
  // unless the person already edited.
  useEffect(() => {
    if (!stored.data) return;
    const drafts = Array.isArray(stored.data.drafts) ? stored.data.drafts : [];
    for (const d of drafts) {
      revisions.current.set(d.id, d.revision);
      snapshots.current.set(
        d.id,
        JSON.stringify({
          manifest: d.manifest,
          environment: d.environment ?? "",
        }),
      );
    }
    const draft = drafts[0];
    if (draft && !latest.current.edited) {
      latest.current.restore(draft.manifest, draft.environment ?? "");
      setRestored(true);
    }
    setReady(true);
  }, [stored.data]);
  useEffect(() => {
    if (stored.error) setError((stored.error as Error).message);
  }, [stored.error]);
  const serialized = JSON.stringify({ manifest, environment });
  useEffect(() => {
    // write-effect: debounced autosave of the edited draft (a write, not a read).
    if (!ready || !edited || snapshots.current.get(manifest.id) === serialized)
      return;
    setPending(true);
    const epoch = generation.current;
    const timer = setTimeout(async () => {
      if (writing.current === epoch) return;
      writing.current = epoch;
      try {
        // A single writer also catches edits made while the previous save was in flight.
        while (generation.current === epoch) {
          const next = latest.current,
            id = next.manifest.id;
          const snapshot = JSON.stringify({
            manifest: next.manifest,
            environment: next.environment,
          });
          if (!next.edited || snapshots.current.get(id) === snapshot) break;
          const result = await request(
            `/api/developer/drafts/${id}`,
            {
              manifest: next.manifest,
              environment: next.environment || null,
              revision: revisions.current.get(id) ?? 0,
            },
            "PUT",
          );
          if (generation.current !== epoch) break;
          revisions.current.set(id, result.revision);
          snapshots.current.set(id, snapshot);
        }
        if (generation.current === epoch) {
          setPending(false);
          setError("");
        }
      } catch (e) {
        if (generation.current === epoch) setError((e as Error).message);
      } finally {
        if (writing.current === epoch) writing.current = null;
      }
    }, 500);
    return () => clearTimeout(timer);
  }, [ready, edited, manifest.id, serialized, request]);
  useEffect(() => {
    const warn = (e: BeforeUnloadEvent) => {
      if (pending || (edited && !ready)) {
        e.preventDefault();
        e.returnValue = "";
      }
    };
    window.addEventListener("beforeunload", warn);
    return () => window.removeEventListener("beforeunload", warn);
  }, [pending, edited, ready]);
  return { ready, pending, error, restored };
}
