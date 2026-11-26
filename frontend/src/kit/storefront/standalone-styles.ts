/** Kit styles for standalone entries (web elements, MCP App views) that load without the Veyndo app stylesheet. */
import tokens from "../tokens/tokens.css?inline";
import ui from "../ui/ui.css?inline";
import storefront from "./storefront.css?inline";

/** Injects the token, ui and part styles into `doc` once. */
export function injectKitStyles(doc: Document = document) {
  if (doc.querySelector("style[data-veyndo-kit]")) return;
  const style = doc.createElement("style");
  style.dataset.veyndoKit = "";
  style.textContent = [tokens, ui, storefront].join("\n");
  doc.head.append(style);
}
