/** Inward catalogue intake and protocol readiness use clear four-language merchant copy. */
import { useLocale } from "../../shared/i18n/i18n";
const words = {
  title: [
    "AI channels & imports",
    "KI-Kanäle & Importe",
    "Canaux IA et imports",
    "Canales IA e importaciones",
  ],
  intro: [
    "Bring existing shops into Veyndo. Let shopping agents hand carts to your checkout.",
    "Bringe bestehende Shops nach Veyndo. Einkaufsagenten übergeben Warenkörbe an deinen Checkout.",
    "Importez des boutiques dans Veyndo. Les agents transmettent les paniers à votre paiement.",
    "Importa tiendas a Veyndo. Los agentes entregan carritos a tu checkout.",
  ],
  back: [
    "Back to sales channels",
    "Zurück zu Verkaufskanälen",
    "Retour aux canaux de vente",
    "Volver a canales de venta",
  ],
  channel: [
    "Sales channel",
    "Verkaufskanal",
    "Canal de vente",
    "Canal de venta",
  ],
  default: [
    "Main sales channel",
    "Haupt-Verkaufskanal",
    "Canal principal",
    "Canal principal",
  ],
  native: [
    "Native protocol readiness",
    "Technischer Protokollstatus",
    "État technique du protocole",
    "Estado técnico del protocolo",
  ],
  cart: [
    "Cart transfer",
    "Warenkorb-Übergabe",
    "Transfert de panier",
    "Transferencia de carrito",
  ],
  consent: [
    "Buyer consent",
    "Käufereinwilligungen",
    "Consentement de l’acheteur",
    "Consentimiento del comprador",
  ],
  ready: ["Implemented", "Implementiert", "Implémenté", "Implementado"],
  unknown: ["Not checked", "Nicht geprüft", "Non vérifié", "Sin verificar"],
  approval: [
    "External approval not verified",
    "Externe Freigabe nicht geprüft",
    "Autorisation externe non vérifiée",
    "Aprobación externa sin verificar",
  ],
  google: [
    "Google Universal Cart",
    "Google Universal Cart",
    "Google Universal Cart",
    "Google Universal Cart",
  ],
  googleHint: [
    "Google can transfer a basket to your Veyndo checkout. Public use requires Google programme and market approval; this screen does not activate it.",
    "Google kann einen Warenkorb an deinen Veyndo-Checkout übergeben. Öffentliche Nutzung erfordert Googles Programm- und Marktfreigabe; diese Ansicht aktiviert sie nicht.",
    "Google peut transmettre un panier au paiement Veyndo. L’usage public exige l’autorisation de Google et du marché ; cet écran ne l’active pas.",
    "Google puede enviar un carrito al checkout Veyndo. El uso público requiere aprobación de Google y del mercado; esta pantalla no lo activa.",
  ],
  guide: [
    "Integration guide",
    "Integrationsanleitung",
    "Guide d’intégration",
    "Guía de integración",
  ],
  shopify: [
    "Shopify → Veyndo",
    "Shopify → Veyndo",
    "Shopify → Veyndo",
    "Shopify → Veyndo",
  ],
  shopifyHint: [
    "Upload your product export, review variants and prices, then create products in a private sandbox. No outgoing Shopify checkout.",
    "Lade deinen Produktexport hoch, prüfe Varianten und Preise und lege Produkte in einer privaten Sandbox an. Kein Wechsel in einen Shopify-Checkout.",
    "Importez votre export, vérifiez variantes et prix, puis créez les produits en sandbox privée. Aucun renvoi au paiement Shopify.",
    "Carga tu exportación, revisa variantes y precios y crea productos en una sandbox privada. Sin redirección al checkout Shopify.",
  ],
  importShopify: [
    "Import Shopify export",
    "Shopify-Export übernehmen",
    "Importer un export Shopify",
    "Importar exportación Shopify",
  ],
  supplier: [
    "Supplier catalogue → Veyndo",
    "Anbieterkatalog → Veyndo",
    "Catalogue fournisseur → Veyndo",
    "Catálogo del proveedor → Veyndo",
  ],
  supplierHint: [
    "Find products with read-only Shop MCP. Review source rights, language and currency. Imported products belong to your Veyndo catalogue and start with zero stock.",
    "Finde Produkte über die lesende Shop-MCP-Suche. Prüfe Quellenrechte, Sprache und Währung. Übernommene Produkte liegen in deinem Veyndo-Katalog und starten mit Bestand null.",
    "Recherchez via Shop MCP en lecture seule. Vérifiez droits, langue et devise. Les produits importés rejoignent Veyndo avec un stock initial nul.",
    "Busca con Shop MCP de solo lectura. Revisa permisos, idioma y moneda. Los productos importados estarán en Veyndo con stock inicial cero.",
  ],
  importSupplier: [
    "Find supplier products",
    "Anbieterprodukte suchen",
    "Rechercher des produits fournisseur",
    "Buscar productos del proveedor",
  ],
  others: [
    "Other shops → Veyndo",
    "Andere Shops → Veyndo",
    "Autres boutiques → Veyndo",
    "Otras tiendas → Veyndo",
  ],
  othersHint: [
    "Use a CSV export or extract drafts from a public product URL. A reviewed import is not a continuous stock or order synchronisation.",
    "Nutze einen CSV-Export oder erstelle Entwürfe aus einer öffentlichen Produkt-URL. Ein geprüfter Import ist kein laufender Bestands- oder Bestellabgleich.",
    "Utilisez un CSV ou une URL publique de produit. Un import vérifié n’est pas une synchronisation continue des stocks ou commandes.",
    "Usa CSV o una URL pública de producto. Una importación revisada no es una sincronización continua de stock o pedidos.",
  ],
  csv: ["Import CSV", "CSV übernehmen", "Importer un CSV", "Importar CSV"],
  url: [
    "Use product URL",
    "Produkt-URL verwenden",
    "Utiliser une URL produit",
    "Usar URL de producto",
  ],
  payment: [
    "Configured payment modes",
    "Konfigurierte Zahlungsmodi",
    "Modes de paiement configurés",
    "Modos de pago configurados",
  ],
  paymentHint: [
    "Configuration is not proof of settlement. Real payment, order email, invoice and refund require provider acceptance.",
    "Konfiguration belegt keine Zahlungsabwicklung. Echte Zahlung, Bestellmail, Rechnung und Erstattung benötigen eine Anbieterabnahme.",
    "La configuration ne prouve pas le règlement. Paiement réel, e-mail, facture et remboursement exigent une validation fournisseur.",
    "La configuración no demuestra el cobro. Pago real, correo, factura y reembolso necesitan validación del proveedor.",
  ],
  permission: [
    "Catalogue read permission is required for this status.",
    "Für diesen Status ist das Recht zum Lesen des Katalogs erforderlich.",
    "Le droit de lecture du catalogue est requis.",
    "Se requiere permiso de lectura del catálogo.",
  ],
  offerTitle: [
    "Signed offers",
    "Signierte Angebote",
    "Offres signées",
    "Ofertas firmadas",
  ],
  offerHint: [
    "A buying agent can ask for the cart's quote with a signature. It checks the signature with your shop's public key and no longer has to trust the price it was shown.",
    "Ein Kaufagent kann das Angebot zum Warenkorb mit Signatur abrufen. Er prüft die Signatur mit dem öffentlichen Schlüssel deines Shops und muss dem angezeigten Preis nicht mehr vertrauen.",
    "Un agent d’achat peut demander le devis du panier avec une signature. Il la vérifie avec la clé publique de votre boutique et n’a plus à croire le prix affiché.",
    "Un agente de compra puede pedir la oferta del carrito con firma. La comprueba con la clave pública de tu tienda y ya no tiene que fiarse del precio mostrado.",
  ],
  offerActive: ["Active", "Aktiv", "Actif", "Activo"],
  offerMissing: [
    "Not set up: the signing key is missing on this installation.",
    "Nicht eingerichtet: Auf dieser Installation fehlt der Signierschlüssel.",
    "Non configuré : la clé de signature manque sur cette installation.",
    "Sin configurar: falta la clave de firma en esta instalación.",
  ],
  offerKey: ["Key", "Schlüssel", "Clé", "Clave"],
  offerValid: ["Valid for", "Gültig für", "Valable", "Válida durante"],
  offerMinutes: [
    "{minutes} minutes",
    "{minutes} Minuten",
    "{minutes} minutes",
    "{minutes} minutos",
  ],
  offerScope: [
    "An offer states what the shop calculated for one cart version. Checkout confirms exactly that version and total or refuses. It reserves no stock and does not freeze a price you change afterwards.",
    "Ein Angebot hält fest, was der Shop für einen Warenkorbstand berechnet hat. Der Checkout bestätigt genau diesen Stand und diese Summe oder lehnt ab. Es reserviert keinen Bestand und friert keinen Preis ein, den du danach änderst.",
    "Une offre indique ce que la boutique a calculé pour une version du panier. Le paiement confirme exactement cette version et ce total, ou refuse. Elle ne réserve aucun stock et ne fige pas un prix que vous modifiez ensuite.",
    "Una oferta indica lo que la tienda calculó para una versión del carrito. El pago confirma exactamente esa versión y ese total, o lo rechaza. No reserva stock ni congela un precio que cambies después.",
  ],
  offerPaths: ["For agents", "Für Agenten", "Pour les agents", "Para agentes"],
} as const;
export function useAgenticChannelText() {
  const { locale } = useLocale();
  return (key: keyof typeof words) =>
    words[key][{ "en-GB": 0, "de-DE": 1, "fr-FR": 2, "es-ES": 3 }[locale]];
}
