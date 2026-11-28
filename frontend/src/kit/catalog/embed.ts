/** Standalone embed entry (`/assets/elements.js`): injects the kit styles once and defines every part element and `<veyndo-document>`. */
import { parts, registerPartElements } from "../storefront";
import { registerDocumentElement } from "./element";
import { injectCatalogStyles } from "./standalone-styles";

// Hosts outside the Veyndo storefront load only this script; styles travel with it.
injectCatalogStyles();
registerPartElements(parts);
registerDocumentElement();
