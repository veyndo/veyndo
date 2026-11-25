/** Permission-scoped consumer request queue and optimistic, recorded review actions. */
import { useRead, useSetRead } from "../shell/useRead";
import { Button, Field, Notice, TextArea } from "../../kit/ui";
import { useState } from "react";
import type { RequestFn } from "../shell/studio-types";
import { useLegalText, type LegalWord } from "../../shared/i18n/legal-i18n";
import type { ConsumerRequest } from "../../shared/legal/legal-types";
export default function ConsumerRequests({
  request,
  canWrite,
}: {
  request: RequestFn;
  canWrite: boolean;
}) {
  const { l } = useLegalText();
  const [selected, setSelected] = useState(""),
    [note, setNote] = useState(""),
    [state, setState] = useState("in_review"),
    [error, setError] = useState(""),
    [busy, setBusy] = useState(false);
  const rowsPath = "/api/merchant/legal/requests";
  const rowsRead = useRead(request, rowsPath);
  const setReadRows = useSetRead(request);
  const rows: ConsumerRequest[] = rowsRead.data?.elements ?? [];
  const rowsLoadError = (rowsRead.error as Error | undefined)?.message;

  const load = () => request(rowsPath).then((v) => setReadRows(rowsPath, v));
  const row = rows.find((r) => r.id === selected);
  return (
    <div className="legal-request-list">
      {(error || rowsLoadError) && (
        <Notice tone="danger">{error || rowsLoadError}</Notice>
      )}
      {!rows.length && <p>{l("emptyRequests")}</p>}
      <div className="legal-request-master">
        {rows.map((r) => (
          <button
            key={r.id}
            aria-pressed={selected === r.id}
            onClick={() => {
              setSelected(r.id);
              setNote(r.data.reviewNote ?? "");
              setState("in_review");
            }}
          >
            <strong>{r.data.name}</strong>
            <span>
              {l(r.kind as LegalWord)} · {l(r.state as LegalWord)}
            </span>
            <small>{r.data.receivedAt}</small>
          </button>
        ))}
      </div>
      {row && (
        <form
          className="legal-request-detail"
          onSubmit={async (e) => {
            e.preventDefault();
            if (!canWrite || busy) return;
            setBusy(true);
            setError("");
            try {
              await request(
                `/api/merchant/legal/requests/${row.id}`,
                { revision: row.revision, state, note },
                "PUT",
              );
              await load();
            } catch (e) {
              setError((e as Error).message);
            } finally {
              setBusy(false);
            }
          }}
        >
          <h3>{row.data.reference || row.data.name}</h3>
          <a href={`mailto:${row.data.email}`}>{row.data.email}</a>
          <p>{row.data.message}</p>
          <small>
            {row.salesChannelId} · #{row.revision}
          </small>
          <Field label={l("reviewNote")}>
            <TextArea
              disabled={!canWrite || busy}
              required
              value={note}
              maxLength={6000}
              onChange={(e) => setNote(e.target.value)}
            />
          </Field>
          <select
            aria-label={l("requests")}
            disabled={!canWrite || busy}
            value={state}
            onChange={(e) => setState(e.target.value)}
          >
            {["in_review", "completed", "declined"].map((s) => (
              <option key={s} value={s}>
                {l(s as LegalWord)}
              </option>
            ))}
          </select>
          <Button
            tone="primary"
            type="submit"
            disabled={!canWrite || busy || !note.trim()}
          >
            {l("reviewSave")}
          </Button>
        </form>
      )}
    </div>
  );
}
