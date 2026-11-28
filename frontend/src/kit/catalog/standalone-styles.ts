/** Kit and catalog styles for the MCP App view, which loads without the Veyndo app stylesheet. */
import tokens from "../tokens/tokens.css?inline";
import motion from "../tokens/motion.css?inline";
import ui from "../ui/ui.css?inline";
import storefront from "../storefront/storefront.css?inline";
import catalog from "./catalog.css?inline";

/** Injects the token, motion, ui, part and document styles into `doc` once. */
export function injectCatalogStyles(doc: Document = document) {
  if (doc.querySelector("style[data-veyndo-kit]")) return;
  const style = doc.createElement("style");
  style.dataset.veyndoKit = "";
  style.textContent = [tokens, motion, ui, storefront, catalog].join("\n");
  doc.head.append(style);
}
