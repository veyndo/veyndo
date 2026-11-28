/** Vocabulary of the catalog renderer and gallery in English, German, French and Spanish: refusal fallbacks, gallery labels and sample text. */
import type { ProblemCode } from "./validate";

const words = {
  refusedTitle: [
    "This part cannot be shown",
    "Dieser Teil kann nicht angezeigt werden",
    "Cette partie ne peut pas être affichée",
    "Esta parte no se puede mostrar",
  ],
  documentTitle: [
    "This view cannot be shown",
    "Diese Ansicht kann nicht angezeigt werden",
    "Cette vue ne peut pas être affichée",
    "Esta vista no se puede mostrar",
  ],
  format: [
    "The view is not in a supported format.",
    "Die Ansicht hat kein unterstütztes Format.",
    "La vue n’est pas dans un format pris en charge.",
    "La vista no tiene un formato compatible.",
  ],
  catalog: [
    "The view was made for another component catalog.",
    "Die Ansicht wurde für einen anderen Komponentenkatalog erstellt.",
    "La vue a été conçue pour un autre catalogue de composants.",
    "La vista se creó para otro catálogo de componentes.",
  ],
  limits: [
    "The view is too large to show safely.",
    "Die Ansicht ist zu groß, um sie sicher anzuzeigen.",
    "La vue est trop grande pour être affichée en toute sécurité.",
    "La vista es demasiado grande para mostrarla con seguridad.",
  ],
  "unknown-component": [
    "It uses a component this app does not know.",
    "Er verwendet eine Komponente, die diese Anwendung nicht kennt.",
    "Elle utilise un composant que cette application ne connaît pas.",
    "Usa un componente que esta aplicación no conoce.",
  ],
  "not-admitted": [
    "This component is not available here.",
    "Diese Komponente ist hier nicht verfügbar.",
    "Ce composant n’est pas disponible ici.",
    "Este componente no está disponible aquí.",
  ],
  "invalid-props": [
    "Its settings are not valid.",
    "Seine Einstellungen sind ungültig.",
    "Ses réglages ne sont pas valides.",
    "Su configuración no es válida.",
  ],
  "unsafe-url": [
    "It links to an address that is not allowed here.",
    "Er verweist auf eine Adresse, die hier nicht erlaubt ist.",
    "Elle renvoie vers une adresse qui n’est pas autorisée ici.",
    "Enlaza a una dirección que no está permitida aquí.",
  ],
  "unsafe-html": [
    "It contains markup or code, which is not allowed.",
    "Er enthält Markup oder Code, was nicht erlaubt ist.",
    "Elle contient du balisage ou du code, ce qui n’est pas autorisé.",
    "Contiene marcado o código, lo que no está permitido.",
  ],
  "data-not-admitted": [
    "It uses data this page did not provide.",
    "Er verwendet Daten, die diese Seite nicht bereitgestellt hat.",
    "Elle utilise des données que cette page n’a pas fournies.",
    "Usa datos que esta página no ha proporcionado.",
  ],
  slot: [
    "Its content is not arranged as allowed.",
    "Sein Inhalt ist nicht wie erlaubt angeordnet.",
    "Son contenu n’est pas disposé comme autorisé.",
    "Su contenido no está organizado como se permite.",
  ],
  placement: [
    "It is placed where it cannot appear.",
    "Er steht an einer Stelle, an der er nicht erscheinen kann.",
    "Elle est placée là où elle ne peut pas apparaître.",
    "Está colocado donde no puede aparecer.",
  ],
  action: [
    "It asks for an action this page does not offer.",
    "Er verlangt eine Aktion, die diese Seite nicht anbietet.",
    "Elle demande une action que cette page ne propose pas.",
    "Pide una acción que esta página no ofrece.",
  ],
  render: [
    "It stopped while being displayed.",
    "Die Anzeige wurde abgebrochen.",
    "Son affichage s’est interrompu.",
    "Su visualización se ha interrumpido.",
  ],
  galleryTitle: [
    "Component gallery",
    "Komponentengalerie",
    "Galerie de composants",
    "Galería de componentes",
  ],
  galleryIntro: [
    "Every catalog component with its states, rendered from documents by the same renderer agents, apps and MCP Apps use.",
    "Alle Katalogkomponenten mit ihren Zuständen, aus Dokumenten mit demselben Renderer dargestellt, den Agenten, Apps und MCP-Apps verwenden.",
    "Chaque composant du catalogue avec ses états, affiché à partir de documents par le même moteur de rendu que les agents, les applications et les applications MCP.",
    "Cada componente del catálogo con sus estados, representado desde documentos con el mismo renderizador que usan los agentes, las apps y las apps MCP.",
  ],
  light: ["Light", "Hell", "Clair", "Claro"],
  dark: ["Dark", "Dunkel", "Sombre", "Oscuro"],
  language: [
    "Sample language",
    "Beispielsprache",
    "Langue des exemples",
    "Idioma de los ejemplos",
  ],
  filter: [
    "Find a component",
    "Komponente suchen",
    "Rechercher un composant",
    "Buscar un componente",
  ],
  surfaces: ["Surfaces", "Oberflächen", "Surfaces", "Superficies"],
  hostData: [
    "host data",
    "Host-Daten",
    "données de l’hôte",
    "datos del anfitrión",
  ],
  codeOnly: ["Code only", "Nur im Code", "Code uniquement", "Solo en código"],
  props: ["Props", "Eigenschaften", "Propriétés", "Propiedades"],
  slots: ["Slots", "Slots", "Emplacements", "Ranuras"],
  events: ["Events", "Ereignisse", "Événements", "Eventos"],
  accessibility: [
    "Accessibility",
    "Barrierefreiheit",
    "Accessibilité",
    "Accesibilidad",
  ],
  document: ["Document", "Dokument", "Document", "Documento"],
  noExamples: [
    "Shown with host data in the storefront; no sample here.",
    "Wird im Shop mit Host-Daten gezeigt; hier ohne Beispiel.",
    "Affiché avec les données de l’hôte dans la boutique ; pas d’exemple ici.",
    "Se muestra con datos del anfitrión en la tienda; aquí sin ejemplo.",
  ],
  body: [
    "Orders keep their own copy of the product.",
    "Bestellungen behalten ihre eigene Kopie des Produkts.",
    "Les commandes conservent leur propre copie du produit.",
    "Los pedidos conservan su propia copia del producto.",
  ],
  hint: [
    "Shown on invoices.",
    "Erscheint auf Rechnungen.",
    "Affiché sur les factures.",
    "Aparece en las facturas.",
  ],
  title: ["Delivery", "Lieferung", "Livraison", "Entrega"],
  card: [
    "Shipping profile",
    "Versandprofil",
    "Profil d’expédition",
    "Perfil de envío",
  ],
  save: ["Save", "Speichern", "Enregistrer", "Guardar"],
  saving: ["Saving", "Wird gespeichert", "Enregistrement", "Guardando"],
  cancel: ["Cancel", "Abbrechen", "Annuler", "Cancelar"],
  more: ["More", "Mehr", "Plus", "Más"],
  delete: ["Delete", "Löschen", "Supprimer", "Eliminar"],
  details: [
    "View products",
    "Produkte ansehen",
    "Voir les produits",
    "Ver productos",
  ],
  draft: ["Draft", "Entwurf", "Brouillon", "Borrador"],
  new: ["New", "Neu", "Nouveau", "Nuevo"],
  paid: ["Paid", "Bezahlt", "Payé", "Pagado"],
  pending: ["Pending", "Ausstehend", "En attente", "Pendiente"],
  failed: ["Failed", "Fehlgeschlagen", "Échec", "Fallido"],
  loading: ["Loading", "Wird geladen", "Chargement", "Cargando"],
  name: ["Name", "Name", "Nom", "Nombre"],
  email: ["Email", "E-Mail", "E-mail", "Correo electrónico"],
  emailError: [
    "Enter a valid email address.",
    "Gib eine gültige E-Mail-Adresse ein.",
    "Saisissez une adresse e-mail valide.",
    "Introduce una dirección de correo válida.",
  ],
  note: ["Note", "Notiz", "Note", "Nota"],
  country: ["Country", "Land", "Pays", "País"],
  germany: ["Germany", "Deutschland", "Allemagne", "Alemania"],
  spain: ["Spain", "Spanien", "Espagne", "España"],
  terms: [
    "I accept the terms",
    "Ich akzeptiere die Bedingungen",
    "J’accepte les conditions",
    "Acepto las condiciones",
  ],
  active: ["Active", "Aktiv", "Actif", "Activo"],
  shipped: [
    "The order was shipped.",
    "Die Bestellung wurde versandt.",
    "La commande a été expédiée.",
    "El pedido se ha enviado.",
  ],
  providerDown: [
    "The payment provider did not answer.",
    "Der Zahlungsanbieter hat nicht geantwortet.",
    "Le prestataire de paiement n’a pas répondu.",
    "El proveedor de pagos no ha respondido.",
  ],
  retry: ["Try again", "Erneut versuchen", "Réessayer", "Reintentar"],
  empty: [
    "No orders yet",
    "Noch keine Bestellungen",
    "Aucune commande",
    "Aún no hay pedidos",
  ],
  emptyBody: [
    "Orders appear here as soon as customers buy.",
    "Bestellungen erscheinen hier, sobald Kundinnen und Kunden kaufen.",
    "Les commandes apparaissent ici dès que les clients achètent.",
    "Los pedidos aparecen aquí en cuanto los clientes compran.",
  ],
  navigation: [
    "Order sections",
    "Bestellbereiche",
    "Sections de commande",
    "Secciones del pedido",
  ],
  overview: ["Overview", "Übersicht", "Aperçu", "Resumen"],
  history: ["History", "Verlauf", "Historique", "Historial"],
  actions: ["Actions", "Aktionen", "Actions", "Acciones"],
  duplicate: ["Duplicate", "Duplizieren", "Dupliquer", "Duplicar"],
  archive: ["Archive", "Archivieren", "Archiver", "Archivar"],
  bag: ["Everyday bag", "Alltagstasche", "Sac du quotidien", "Bolso diario"],
  bagCategory: ["Bags", "Taschen", "Sacs", "Bolsos"],
  standard: [
    "Standard delivery",
    "Standardversand",
    "Livraison standard",
    "Envío estándar",
  ],
} as const;

export type CatalogWord = keyof typeof words;
/** The gallery's sample languages; French and Spanish ship in the vocabulary too. */
export const galleryLocales = ["en-GB", "de-DE", "fr-FR", "es-ES"] as const;

const index = (locale: string) =>
  ({ de: 1, fr: 2, es: 3 })[
    locale.slice(0, 2).toLowerCase() as "de" | "fr" | "es"
  ] ?? 0;

/** Text of the catalog renderer and gallery in `locale`, English for other languages. */
export function catalogText(locale: string) {
  const i = index(locale);
  return (key: CatalogWord) => words[key][i];
}

/** The fallback message of a refusal code. */
export const refusalText = (locale: string, code: ProblemCode | "render") =>
  catalogText(locale)(code);

/** Sample text for examples; unknown keys are returned unchanged so a missing key shows in the gallery test. */
export const sampleText = (locale: string) => {
  const t = catalogText(locale);
  return (key: string) =>
    Object.hasOwn(words, key) ? t(key as CatalogWord) : key;
};
