/** Vocabulary for supplier links and reorder proposals in EN/DE/FR/ES; figures and names are parameters. */
import { useLocale } from "../../shared/i18n/i18n";
const words = {
  title: [
    "Reorder in the Veyndo network",
    "Nachbestellen im Veyndo-Netz",
    "Réassort dans le réseau Veyndo",
    "Reponer en la red Veyndo",
  ],
  intro: [
    "Link a product to the shop you buy it from. When stock falls to the reorder point, Veyndo fetches a signed offer from that shop and you decide. Nothing is ordered by itself.",
    "Verknüpfe ein Produkt mit dem Shop, bei dem du es einkaufst. Fällt der Bestand auf den Meldebestand, holt Veyndo ein signiertes Angebot dieses Shops und du entscheidest. Von selbst wird nichts bestellt.",
    "Associez un produit à la boutique où vous l’achetez. Quand le stock atteint le seuil, Veyndo demande une offre signée à cette boutique et vous décidez. Rien n’est commandé automatiquement.",
    "Vincula un producto con la tienda donde lo compras. Cuando el stock baja al punto de pedido, Veyndo pide una oferta firmada a esa tienda y tú decides. No se pide nada por sí solo.",
  ],
  scope: [
    "Suppliers must be shops on this Veyndo installation.",
    "Lieferanten müssen Shops auf dieser Veyndo-Installation sein.",
    "Les fournisseurs doivent être des boutiques de cette installation Veyndo.",
    "Los proveedores deben ser tiendas de esta instalación de Veyndo.",
  ],
  ceiling: [
    "Ceiling per reorder · {currency}",
    "Höchstbetrag je Nachbestellung · {currency}",
    "Plafond par réassort · {currency}",
    "Tope por reposición · {currency}",
  ],
  ceilingHint: [
    "Without a ceiling nothing can be approved.",
    "Ohne Höchstbetrag lässt sich nichts freigeben.",
    "Sans plafond, rien ne peut être validé.",
    "Sin tope no se puede aprobar nada.",
  ],
  product: [
    "Your product ID",
    "Deine Produkt-ID",
    "Votre ID produit",
    "Tu ID de producto",
  ],
  supplier: [
    "Supplier shop",
    "Lieferanten-Shop",
    "Boutique fournisseur",
    "Tienda proveedora",
  ],
  supplierProduct: [
    "Product ID at the supplier",
    "Produkt-ID beim Lieferanten",
    "ID produit chez le fournisseur",
    "ID de producto en el proveedor",
  ],
  point: [
    "Reorder point",
    "Meldebestand",
    "Seuil de réassort",
    "Punto de pedido",
  ],
  quantity: [
    "Reorder quantity",
    "Bestellmenge",
    "Quantité à commander",
    "Cantidad a pedir",
  ],
  add: [
    "Add supplier link",
    "Lieferantenverknüpfung hinzufügen",
    "Ajouter un lien fournisseur",
    "Añadir vínculo de proveedor",
  ],
  remove: ["Remove", "Entfernen", "Supprimer", "Quitar"],
  save: [
    "Save supplier links",
    "Lieferantenverknüpfungen speichern",
    "Enregistrer les liens fournisseur",
    "Guardar vínculos de proveedor",
  ],
  saveFirst: [
    "Save your changes before asking for a proposal.",
    "Speichere deine Änderungen, bevor du einen Vorschlag holst.",
    "Enregistrez vos modifications avant de demander une proposition.",
    "Guarda los cambios antes de pedir una propuesta.",
  ],
  propose: [
    "Get reorder proposal",
    "Nachbestell-Vorschlag holen",
    "Obtenir une proposition de réassort",
    "Obtener propuesta de reposición",
  ],
  none: [
    "No product is at or below its reorder point. Products with enough stock: {count}.",
    "Kein Produkt ist am oder unter dem Meldebestand. Produkte mit genug Bestand: {count}.",
    "Aucun produit n’est au seuil ou en dessous. Produits avec assez de stock : {count}.",
    "Ningún producto está en su punto de pedido o por debajo. Productos con stock suficiente: {count}.",
  ],
  line: [
    "Stock {stock}, reorder point {point}. Order {quantity} from {supplier}.",
    "Bestand {stock}, Meldebestand {point}. {quantity} Stück bei {supplier} bestellen.",
    "Stock {stock}, seuil {point}. Commander {quantity} chez {supplier}.",
    "Stock {stock}, punto de pedido {point}. Pedir {quantity} a {supplier}.",
  ],
  offer: [
    "Signed offer: {total}, valid until {time}.",
    "Signiertes Angebot: {total}, gültig bis {time}.",
    "Offre signée : {total}, valable jusqu’à {time}.",
    "Oferta firmada: {total}, válida hasta {time}.",
  ],
  within: [
    "Within the ceiling",
    "Im Höchstbetrag",
    "Dans le plafond",
    "Dentro del tope",
  ],
  over: [
    "Above the ceiling or no ceiling set",
    "Über dem Höchstbetrag oder kein Höchstbetrag gesetzt",
    "Au-dessus du plafond ou aucun plafond défini",
    "Por encima del tope o sin tope definido",
  ],
  otherCurrency: [
    "The supplier quotes in another currency than your shop.",
    "Der Lieferant bietet in einer anderen Währung an als dein Shop.",
    "Le fournisseur propose une autre devise que votre boutique.",
    "El proveedor ofrece en otra moneda que tu tienda.",
  ],
  lowStock: [
    "The supplier currently shows less stock than this quantity.",
    "Der Lieferant zeigt aktuell weniger Bestand als diese Menge.",
    "Le fournisseur affiche actuellement moins de stock que cette quantité.",
    "El proveedor muestra ahora menos stock que esta cantidad.",
  ],
  refused: [
    "The supplier shop gave no offer:",
    "Der Lieferanten-Shop hat kein Angebot gegeben:",
    "La boutique fournisseur n’a pas fait d’offre :",
    "La tienda proveedora no dio oferta:",
  ],
  missing: [
    "This product no longer exists in your shop.",
    "Dieses Produkt gibt es in deinem Shop nicht mehr.",
    "Ce produit n’existe plus dans votre boutique.",
    "Este producto ya no existe en tu tienda.",
  ],
  approve: [
    "Approve and open supplier checkout",
    "Freigeben und Lieferanten-Checkout öffnen",
    "Valider et ouvrir le paiement fournisseur",
    "Aprobar y abrir el pago del proveedor",
  ],
  approved: [
    "Approved. Nothing is ordered yet: complete the purchase in the supplier's checkout. The link works once and for ten minutes.",
    "Freigegeben. Bestellt ist noch nichts: Schließe den Kauf im Checkout des Lieferanten ab. Der Link gilt einmal und zehn Minuten.",
    "Validé. Rien n’est encore commandé : terminez l’achat dans le paiement du fournisseur. Le lien fonctionne une fois, pendant dix minutes.",
    "Aprobado. Aún no se ha pedido nada: completa la compra en el pago del proveedor. El enlace funciona una vez y durante diez minutos.",
  ],
  open: [
    "Open supplier checkout",
    "Lieferanten-Checkout öffnen",
    "Ouvrir le paiement fournisseur",
    "Abrir el pago del proveedor",
  ],
  readOnly: [
    "Your role can view supplier links but not change or approve them.",
    "Deine Rolle kann Lieferantenverknüpfungen ansehen, aber nicht ändern oder freigeben.",
    "Votre rôle peut consulter les liens fournisseur, mais pas les modifier ni les valider.",
    "Tu rol puede ver los vínculos de proveedor, pero no cambiarlos ni aprobarlos.",
  ],
} as const;
export type SupplyWord = keyof typeof words;
export function useSupplyText() {
  const { locale } = useLocale();
  const index =
    ({ en: 0, de: 1, fr: 2, es: 3 } as Record<string, number>)[
      locale.slice(0, 2)
    ] ?? 0;
  return (key: SupplyWord, values: Record<string, string | number> = {}) =>
    Object.entries(values).reduce<string>(
      (text, [name, value]) => text.replace(`{${name}}`, String(value)),
      words[key][index],
    );
}
