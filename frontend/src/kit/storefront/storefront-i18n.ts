/** Vocabulary of the storefront parts in English, German, French and Spanish; hosts pass only the locale. */
const words = {
  details: [
    "View product",
    "Produkt ansehen",
    "Voir le produit",
    "Ver producto",
  ],
  available: ["available", "verfügbar", "disponibles", "disponibles"],
  soldOut: ["Sold out", "Ausverkauft", "Épuisé", "Agotado"],
  noImage: ["No image", "Kein Bild", "Pas d’image", "Sin imagen"],
  gross: ["incl. VAT", "inkl. MwSt.", "TVA incluse", "IVA incluido"],
  net: ["excl. VAT", "zzgl. MwSt.", "hors TVA", "IVA no incluido"],
  addToCart: [
    "Add to cart",
    "In den Warenkorb",
    "Ajouter au panier",
    "Añadir al carrito",
  ],
  quantity: ["Quantity", "Menge", "Quantité", "Cantidad"],
  cart: ["Cart", "Warenkorb", "Panier", "Carrito"],
  emptyCart: [
    "Your cart is empty.",
    "Dein Warenkorb ist leer.",
    "Votre panier est vide.",
    "Tu carrito está vacío.",
  ],
  subtotal: ["Subtotal", "Zwischensumme", "Sous-total", "Subtotal"],
  shipping: ["Shipping", "Versand", "Livraison", "Envío"],
  total: ["Total", "Gesamt", "Total", "Total"],
  checkout: [
    "Go to checkout",
    "Zur Kasse",
    "Passer à la caisse",
    "Ir a la caja",
  ],
  checkoutSteps: [
    "Checkout steps",
    "Schritte zur Bestellung",
    "Étapes de commande",
    "Pasos del pedido",
  ],
  details_step: ["Your details", "Deine Daten", "Vos coordonnées", "Tus datos"],
  review_step: ["Review", "Prüfen", "Vérification", "Revisión"],
  complete_step: ["Complete", "Abschluss", "Confirmation", "Confirmación"],
  account: ["Your account", "Dein Konto", "Votre compte", "Tu cuenta"],
  orders: [
    "Recent orders",
    "Letzte Bestellungen",
    "Commandes récentes",
    "Pedidos recientes",
  ],
  noOrders: [
    "No orders yet.",
    "Noch keine Bestellungen.",
    "Pas encore de commande.",
    "Aún no hay pedidos.",
  ],
  summary: ["Your order", "Deine Bestellung", "Votre commande", "Tu pedido"],
  remove: ["Remove", "Entfernen", "Retirer", "Eliminar"],
  tax: ["VAT", "Mehrwertsteuer", "TVA", "IVA"],
  discount: ["Discount", "Rabatt", "Remise", "Descuento"],
  deliveryWindow: [
    "Delivery: {from} — {to}",
    "Lieferung: {from} — {to}",
    "Livraison : {from} — {to}",
    "Entrega: {from} — {to}",
  ],
  delivery: [
    "Delivery in {days} days",
    "Lieferung in {days} Tagen",
    "Livraison sous {days} jours",
    "Entrega en {days} días",
  ],
} as const;
export type StorefrontWord = keyof typeof words;
const index = (locale: string) =>
  locale.startsWith("de")
    ? 1
    : locale.startsWith("fr")
      ? 2
      : locale.startsWith("es")
        ? 3
        : 0;
/** The parts' text in `locale`, with `{parameters}` filled. */
export function storefrontText(locale: string) {
  const i = index(locale);
  return (key: StorefrontWord, values: Record<string, string | number> = {}) =>
    words[key][i].replace(/\{(\w+)\}/g, (_, name: string) =>
      String(values[name] ?? ""),
    );
}
/** Money in the shop's currency, formatted for `locale`. */
export const money = (locale: string, currency: string, amount: number) =>
  new Intl.NumberFormat(locale, { style: "currency", currency }).format(amount);
