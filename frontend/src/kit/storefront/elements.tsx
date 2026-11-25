/** Embeddable web elements from part contracts: validated `props` attributes, buyer intents as DOM events. */
import { createRoot, type Root } from "react-dom/client";
import type { AnyPart, PartEvents } from "./contract";

export const addToCartEvent = "veyndo:add-to-cart";
export const checkoutEvent = "veyndo:checkout";
export const quantityEvent = "veyndo:quantity";
export const openEvent = "veyndo:open";

/**
 * Defines `<veyndo-<id>>` for each part. The element reads JSON from its `props`
 * attribute (and `locale`), validates it with the part's schema and renders the
 * part in place of any server-rendered fallback children (island upgrade). Invalid
 * props leave the fallback in place and mark the element `data-invalid`.
 */
export function registerPartElements(
  parts: readonly AnyPart[],
  registry: CustomElementRegistry = customElements,
) {
  for (const part of parts) {
    if (registry.get(part.element)) continue;
    registry.define(
      part.element,
      class extends HTMLElement {
        static observedAttributes = ["props", "locale"];
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
          // React may still be rendering; unmount after the current task.
          if (root) queueMicrotask(() => root.unmount());
        }
        private render() {
          let raw: unknown = null;
          try {
            raw = JSON.parse(this.getAttribute("props") ?? "{}");
          } catch {
            raw = null;
          }
          const locale = this.getAttribute("locale");
          if (raw && typeof raw === "object" && locale && !("locale" in raw))
            raw = { ...raw, locale };
          const result = part.parse(raw);
          if (!result.ok) {
            this.dataset.invalid = "true";
            this.dataset.issues = result.issues.join("; ").slice(0, 500);
            return;
          }
          delete this.dataset.invalid;
          delete this.dataset.issues;
          const emit = (type: string, detail?: unknown) =>
            this.dispatchEvent(
              new CustomEvent(type, { detail, bubbles: true, composed: true }),
            );
          const Component = part.Component;
          this.root ??= createRoot(this);
          this.root.render(
            <Component
              {...(result.props as object)}
              onAddToCart={(
                detail: Parameters<NonNullable<PartEvents["onAddToCart"]>>[0],
              ) => emit(addToCartEvent, detail)}
              onCheckout={() => emit(checkoutEvent)}
              onQuantity={(
                detail: Parameters<NonNullable<PartEvents["onQuantity"]>>[0],
              ) => emit(quantityEvent, detail)}
              onOpen={(
                detail: Parameters<NonNullable<PartEvents["onOpen"]>>[0],
              ) => emit(openEvent, detail)}
            />,
          );
        }
      },
    );
  }
}
