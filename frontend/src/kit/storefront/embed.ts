/** Standalone embed entry (`/assets/elements.js`): injects the kit styles once and defines every storefront part as a web element. */
import tokens from "../tokens/tokens.css?inline";
import ui from "../ui/ui.css?inline";
import storefront from "./storefront.css?inline";
import { parts, registerPartElements } from "./index";

// Hosts outside the Veyndo storefront load only this script; styles travel with it.
if (!document.querySelector("style[data-veyndo-elements]")) {
  const style = document.createElement("style");
  style.dataset.veyndoElements = "";
  style.textContent = [tokens, ui, storefront].join("\n");
  document.head.append(style);
}
registerPartElements(parts);
