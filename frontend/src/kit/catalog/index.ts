/** Public interface of kit/catalog: the versioned component catalog, document validation, the one renderer and the gallery. */
import "../tokens";
import "./catalog.css";
export {
  catalogId,
  catalogVersion,
  limits,
  entries,
  entryByName,
  catalogDocument,
  propTypesSource,
} from "./catalog";
export { surfaces, defineEntry } from "./define";
export type {
  CatalogEntry,
  Example,
  ExampleNode,
  Kind,
  SlotRule,
  EventRule,
  Accessibility,
  Surface,
} from "./define";
export { plainText, safeUrl, identifier } from "./fields";
export { readDocument } from "./document";
export type { Format, RawNode, ReadResult } from "./document";
export { validateDocument, pointer } from "./validate";
export type {
  Admission,
  HostComponent,
  Problem,
  ProblemCode,
  ValidNode,
  Validation,
} from "./validate";
export { CatalogView } from "./CatalogView";
export type { CatalogViewProps, DocumentAction } from "./CatalogView";
export {
  Gallery,
  exampleDocument,
  exampleAdmission,
  typeLabel,
} from "./Gallery";
export {
  catalogText,
  refusalText,
  sampleText,
  galleryLocales,
} from "./catalog-i18n";
export type { CatalogWord } from "./catalog-i18n";
export {
  documentKey,
  renderMcpDocument,
  startCatalogMcpView,
} from "./mcp-view";
export {
  registerDocumentElement,
  documentElement,
  actionEvent,
} from "./element";
