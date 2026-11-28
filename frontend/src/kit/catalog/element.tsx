/** `<veyndo-document>`: the storefront surface's web element; it renders a catalog document with the data and actions its host page admits. */
import { createRoot, type Root } from "react-dom/client";
import { CatalogView, type DocumentAction } from "./CatalogView";

export const documentElement = "veyndo-document";
/** Fired on the element (bubbling, composed) when a control reports an admitted action. */
export const actionEvent = "veyndo:action";

const json = (value: string | null): unknown => {
  if (value === null) return undefined;
  try {
    return JSON.parse(value);
  } catch {
    return undefined;
  }
};
const list = (value: string | null) =>
  (value ?? "").split(/\s+/).filter(Boolean);

/**
 * Defines `<veyndo-document document='…' data='…' actions="add open" origins="https://shop.example" locale="de-DE">`.
 * `document` is a `veyndo-ui/1` or A2UI JSON document or OpenUI Lang text, typically
 * from an agent; `data`, `actions` and `origins` are what the embedding page admits.
 * A refused document shows the localized fallback in place.
 */
export function registerDocumentElement(
  registry: CustomElementRegistry = customElements,
) {
  if (registry.get(documentElement)) return;
  registry.define(
    documentElement,
    class extends HTMLElement {
      static observedAttributes = [
        "document",
        "data",
        "actions",
        "origins",
        "locale",
      ];
      private root?: Root;
      connectedCallback() {
        this.render();
      }
      attributeChangedCallback() {
        if (this.isConnected) this.render();
      }
      disconnectedCallback() {
        const root = this.root;
        this.root = undefined;
        if (root) queueMicrotask(() => root.unmount());
      }
      private render() {
        const raw = this.getAttribute("document") ?? "";
        const document =
          raw.trim().startsWith("{") || raw.trim().startsWith("[")
            ? (json(raw) ?? raw)
            : raw;
        this.root ??= createRoot(this);
        this.root.render(
          <CatalogView
            document={document}
            admission={{
              surface: "storefront",
              locale:
                this.getAttribute("locale") ?? navigator.language ?? "en-GB",
              data: json(this.getAttribute("data")),
              actions: list(this.getAttribute("actions")),
              origins: list(this.getAttribute("origins")).filter((origin) =>
                /^https:\/\/[^/]+$/.test(origin),
              ),
            }}
            onAction={(detail: DocumentAction) =>
              this.dispatchEvent(
                new CustomEvent(actionEvent, {
                  detail,
                  bubbles: true,
                  composed: true,
                }),
              )
            }
          />,
        );
      }
    },
  );
}
