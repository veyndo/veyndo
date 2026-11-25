/** Optional image-provider jobs create private previews; applying a reviewed image is explicit and revision checked. */
import { Button, Notice } from "../../kit/ui";
import { useEffect, useState } from "react";
import { useWorkspaceText } from "../../shared/i18n/workspace-i18n";
import type { RequestFn } from "../shell/studio-types";
import { useImageJob } from "./useImageJob";
import type { GalleryImage } from "./media-model";
export default function AiImageStudio({
  productId,
  revision,
  sourceId,
  request,
  onImage,
  disabled,
}: {
  productId: string;
  revision: number;
  sourceId?: string;
  request: RequestFn;
  onImage: (m: GalleryImage) => void;
  disabled: boolean;
}) {
  const { w } = useWorkspaceText();
  const [configured, setConfigured] = useState(false),
    [prompt, setPrompt] = useState("");
  const { job, busy, error, start, apply } = useImageJob(
    request,
    productId,
    w("imageFailed"),
  );
  useEffect(() => {
    let active = true;
    setConfigured(false);
    request("/api/merchant/media/provider")
      .then((v) => {
        if (active) setConfigured(v.configured);
      })
      .catch(() => {});
    return () => {
      active = false;
    };
  }, [request]);
  const create = (mode: string) =>
    start({
      prompt,
      mode,
      sourceId: mode === "optimize" ? sourceId : null,
      revision,
    });
  const waiting = job && ["queued", "processing"].includes(job.state);
  return (
    <details className="media-ai-studio">
      <summary>✦ {w("aiImages")}</summary>
      <p>{w(configured ? "aiHint" : "aiDisabled")}</p>
      {configured && disabled && <p>{w("saveBeforeImage")}</p>}
      <label>
        {w("prompt")}
        <textarea
          value={prompt}
          maxLength={2000}
          rows={3}
          onChange={(e) => setPrompt(e.target.value)}
          disabled={!configured || busy || waiting || disabled}
        />
      </label>
      <div className="media-actions">
        <Button
          tone="primary"
          type="button"
          disabled={
            !configured || busy || waiting || disabled || !prompt.trim()
          }
          onClick={() => void create("generate")}
        >
          {w("generate")}
        </Button>
        <Button
          type="button"
          disabled={
            !configured ||
            !sourceId ||
            busy ||
            waiting ||
            disabled ||
            !prompt.trim()
          }
          onClick={() => void create("optimize")}
        >
          {w("optimize")}
        </Button>
      </div>
      {waiting && <p role="status">{w("generating")}</p>}
      {error && <Notice tone="danger">{error}</Notice>}
      {job?.state === "ready" && (
        <div className="media-ai-preview">
          <img src={job.preview} alt={w("aiImages")} />
          <Button
            tone="primary"
            type="button"
            disabled={busy || disabled}
            onClick={() => void apply(revision, onImage)}
          >
            {w("applyImage")}
          </Button>
        </div>
      )}
    </details>
  );
}
