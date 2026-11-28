/** Catalog documents in the MCP App view: a tool result's `ai.veyndo/ui` payload renders through CatalogView on the mcp-app surface. */
import type { MouseEvent } from "react";
import { startMcpView, type RenderMeta } from "../storefront";
import { CatalogView } from "./CatalogView";
import { injectCatalogStyles } from "./standalone-styles";

/** Where the server puts a document of a tool result; `_meta` never reaches the model. */
export const documentKey = "ai.veyndo/ui";

type DocumentPayload = {
  document: unknown;
  /** Data the server resolved for the document under the caller's rights. */
  data?: unknown;
  locale?: string;
  /** The shop origins its links and images may use. */
  origins?: string[];
};

const isPayload = (value: unknown): value is DocumentPayload =>
  typeof value === "object" && value !== null && "document" in value;

/** Renders the document payload of a tool result, or `null` when the result carries none. */
export const renderMcpDocument: RenderMeta = (meta, { locale, open }) => {
  const payload = meta[documentKey];
  if (!isPayload(payload)) return null;
  // Links leave the sandbox only through the host (`ui/open-link`), and only https ones.
  const intercept = (event: MouseEvent) => {
    const anchor = (event.target as Element).closest?.("a[href]");
    if (!anchor) return;
    event.preventDefault();
    const base = payload.origins?.find((origin) =>
      origin.startsWith("https://"),
    );
    const href = anchor.getAttribute("href")!;
    if (href.startsWith("https://")) open(href);
    else if (base) open(new URL(href, base).href);
  };
  return (
    <div onClickCapture={intercept}>
      <CatalogView
        document={payload.document}
        admission={{
          surface: "mcp-app",
          locale: payload.locale ?? locale,
          data: payload.data,
          origins: (payload.origins ?? []).filter((origin) =>
            origin.startsWith("https://"),
          ),
        }}
      />
    </div>
  );
};

/** Starts the MCP App view with catalog documents and kit parts. */
export const startCatalogMcpView = (target: Window = window.parent) =>
  startMcpView(target, {
    renderMeta: renderMcpDocument,
    injectStyles: injectCatalogStyles,
  });
