/** MCP App view (`ui://veyndo/kit/storefront`): renders kit parts from a commerce tool result inside the host sandbox. */
import { createRoot } from "react-dom/client";
import { useEffect, useState } from "react";
import { injectKitStyles } from "./standalone-styles";
import { partById, storefrontText } from "./index";

/** The MCP Apps bridge version this view speaks. */
export const bridgeVersion = "2026-01-26";
/** Where the server puts the part payload of a tool result; `_meta` never reaches the model. */
export const partKey = "ai.veyndo/part";
type Payload = {
  part: string;
  locale?: string;
  props?: unknown;
  items?: unknown[];
  /** Absolute shop URLs per product ID, where buying continues natively. */
  links?: Record<string, string>;
  /** The shop's native checkout URL. */
  checkout?: string;
};
type Message = {
  jsonrpc?: string;
  id?: string | number;
  method?: string;
  params?: Record<string, unknown>;
  result?: Record<string, unknown>;
};

/** Valid props of a payload; invalid or unknown parts yield nothing. */
export function payloadProps(payload: Payload | undefined) {
  const part = payload && partById(payload.part);
  if (!part || !payload) return { part: undefined, props: [] as object[] };
  const items = payload.items ?? (payload.props ? [payload.props] : []);
  const props = items
    .slice(0, 24)
    .map((item) => part.parse(item))
    .flatMap((r) => (r.ok ? [r.props as object] : []));
  return { part, props };
}

/** Only absolute https links leave the sandbox, through the host. */
const linkable = (url: unknown): url is string =>
  typeof url === "string" && url.startsWith("https://");

export function View({
  send,
  subscribe,
}: {
  send: (message: Message) => void;
  subscribe: (listener: (message: Message) => void) => () => void;
}) {
  const [locale, setLocale] = useState(navigator.language || "en-GB");
  const [payload, setPayload] = useState<Payload>();
  const [received, setReceived] = useState(false);
  useEffect(
    () =>
      subscribe((message) => {
        if (message.id === "veyndo-init" && message.result) {
          const context = (message.result.hostContext ?? {}) as {
            theme?: string;
            locale?: string;
          };
          if (context.theme === "dark" || context.theme === "light")
            document.documentElement.dataset.theme = context.theme;
          if (context.locale) setLocale(context.locale);
          send({ method: "ui/notifications/initialized" });
        }
        if (message.method === "ui/notifications/host-context-changed") {
          const theme = message.params?.theme;
          if (theme === "dark" || theme === "light")
            document.documentElement.dataset.theme = theme;
        }
        if (message.method === "ui/notifications/tool-result") {
          const meta = (message.params?._meta ?? {}) as Record<string, unknown>;
          setPayload(meta[partKey] as Payload | undefined);
          setReceived(true);
        }
        if (
          message.method === "ui/resource-teardown" &&
          message.id !== undefined
        )
          send({ id: message.id, result: {} });
      }),
    [send, subscribe],
  );
  const t = storefrontText(payload?.locale ?? locale);
  const { part, props } = payloadProps(payload);
  if (!part || !props.length)
    return <p role="status">{t(received ? "unavailable" : "waiting")}</p>;
  const open = (url: unknown) => {
    if (linkable(url))
      send({
        id: `open-${Date.now()}`,
        method: "ui/open-link",
        params: { url },
      });
  };
  const Component = part.Component;
  return (
    <div className={props.length > 1 ? "vy-sf-grid" : undefined}>
      {props.map((item, index) => (
        <Component
          key={index}
          {...item}
          // Buying continues in the shop's own product page and checkout.
          onAddToCart={({ productId }: { productId: string }) =>
            open(payload?.links?.[productId])
          }
          onCheckout={() => open(payload?.checkout)}
        />
      ))}
    </div>
  );
}

/**
 * A host channel that keeps messages until the view subscribes: the host may
 * answer `ui/initialize` before React has mounted.
 */
export function channel(target: Window, source: Window = window) {
  const listeners = new Set<(message: Message) => void>();
  const pending: Message[] = [];
  source.addEventListener("message", (event) => {
    if (event.source !== target || event.data?.jsonrpc !== "2.0") return;
    if (!listeners.size) pending.push(event.data as Message);
    listeners.forEach((listener) => listener(event.data as Message));
  });
  const send = (message: Message) =>
    target.postMessage({ jsonrpc: "2.0", ...message }, "*");
  const subscribe = (listener: (message: Message) => void) => {
    listeners.add(listener);
    pending.splice(0).forEach(listener);
    return () => {
      listeners.delete(listener);
    };
  };
  return { send, subscribe };
}

/** Starts the view in this document and announces it to the host. */
export function start(target: Window = window.parent) {
  injectKitStyles();
  const { send, subscribe } = channel(target);
  const root = document.getElementById("root")!;
  createRoot(root).render(<View send={send} subscribe={subscribe} />);
  new ResizeObserver(() =>
    send({
      method: "ui/notifications/size-changed",
      params: {
        height: document.body.offsetHeight,
        width: document.documentElement.clientWidth,
      },
    }),
  ).observe(document.body);
  send({
    id: "veyndo-init",
    method: "ui/initialize",
    params: {
      appInfo: { name: "veyndo-storefront-parts", version: "1.0.0" },
      appCapabilities: {},
      protocolVersion: bridgeVersion,
    },
  });
}
