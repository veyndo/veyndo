/** Standalone embed entry (`/assets/elements.js`): injects the kit styles once and defines every storefront part as a web element. */
import { injectKitStyles } from "./standalone-styles";
import { parts, registerPartElements } from "./index";

// Hosts outside the Veyndo storefront load only this script; styles travel with it.
injectKitStyles();
registerPartElements(parts);
