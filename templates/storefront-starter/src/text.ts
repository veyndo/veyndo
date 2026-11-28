/** Storefront interface vocabulary in English, German, Spanish and French; product content comes from the shop in its own language. */
export const LOCALES = { en: "en-GB", de: "de-DE", es: "es-ES", fr: "fr-FR" } as const;
export type Language = keyof typeof LOCALES;

export const TEXT: Record<Language, Record<"catalog" | "back" | "inStock" | "soldOut" | "delivery" | "empty" | "failed" | "language", string>> = {
  en: { catalog: "Catalog", back: "← All products", inStock: "In stock", soldOut: "Sold out", delivery: "Delivery in {days} days", empty: "This shop has no products yet.", failed: "The shop could not be reached.", language: "Language" },
  de: { catalog: "Katalog", back: "← Alle Produkte", inStock: "Auf Lager", soldOut: "Ausverkauft", delivery: "Lieferung in {days} Tagen", empty: "Dieser Shop hat noch keine Produkte.", failed: "Der Shop ist nicht erreichbar.", language: "Sprache" },
  es: { catalog: "Catálogo", back: "← Todos los productos", inStock: "Disponible", soldOut: "Agotado", delivery: "Entrega en {days} días", empty: "Esta tienda aún no tiene productos.", failed: "No se pudo contactar con la tienda.", language: "Idioma" },
  fr: { catalog: "Catalogue", back: "← Tous les produits", inStock: "En stock", soldOut: "Épuisé", delivery: "Livraison en {days} jours", empty: "Cette boutique n’a pas encore de produits.", failed: "La boutique est injoignable.", language: "Langue" },
};

export function language(value: string | null | undefined): Language {
  const short = (value ?? "").slice(0, 2).toLowerCase();
  return short in LOCALES ? (short as Language) : "en";
}
