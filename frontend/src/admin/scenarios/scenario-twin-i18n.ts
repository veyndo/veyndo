/** Shop twin vocabulary: readiness steps of a private copy in EN/DE/FR/ES; counts and country names are parameters. */
import { useLocale } from "../../shared/i18n/i18n";
const words = {
  twinTitle: [
    "Shop twin",
    "Shop-Zwilling",
    "Jumeau de la boutique",
    "Gemelo de la tienda",
  ],
  twinIntro: [
    "This private copy behaves like your shop. Test customers and attackers buy here; your live shop notices nothing.",
    "Diese private Kopie verhält sich wie dein Shop. Testkunden und Angreifer kaufen hier, dein Live-Shop merkt davon nichts.",
    "Cette copie privée se comporte comme votre boutique. Clients de test et attaquants achètent ici ; la boutique en ligne ne remarque rien.",
    "Esta copia privada se comporta como tu tienda. Clientes de prueba y atacantes compran aquí; la tienda en vivo no nota nada.",
  ],
  twinReady: [
    "Ready to test",
    "Bereit zum Testen",
    "Prêt à tester",
    "Listo para probar",
  ],
  twinNotReady: [
    "Not ready yet",
    "Noch nicht bereit",
    "Pas encore prêt",
    "Aún no está listo",
  ],
  twinLoading: [
    "Checking the copy…",
    "Kopie wird geprüft…",
    "Vérification de la copie…",
    "Comprobando la copia…",
  ],
  twinCatalogue: ["Catalog", "Katalog", "Catalogue", "Catálogo"],
  twinCatalogueOk: [
    "Purchasable items in this copy: {count}.",
    "Kaufbare Artikel in dieser Kopie: {count}.",
    "Articles achetables dans cette copie : {count}.",
    "Artículos comprables en esta copia: {count}.",
  ],
  twinCatalogueEmpty: [
    "This copy has no products yet. Bring your catalog from Shopify, Shopware, WooCommerce or a CSV file.",
    "Diese Kopie hat noch keine Produkte. Hol deinen Katalog aus Shopify, Shopware, WooCommerce oder einer CSV-Datei.",
    "Cette copie n’a pas encore de produits. Importez votre catalogue depuis Shopify, Shopware, WooCommerce ou un fichier CSV.",
    "Esta copia aún no tiene productos. Trae tu catálogo desde Shopify, Shopware, WooCommerce o un archivo CSV.",
  ],
  twinChannelEmpty: [
    "The copy has products, but none is visible in the selected sales channel.",
    "Die Kopie hat Produkte, aber im gewählten Verkaufskanal ist keines sichtbar.",
    "La copie a des produits, mais aucun n’est visible dans le canal de vente choisi.",
    "La copia tiene productos, pero ninguno es visible en el canal de venta elegido.",
  ],
  twinBring: [
    "Bring catalog",
    "Katalog holen",
    "Importer le catalogue",
    "Traer catálogo",
  ],
  twinStock: ["Test stock", "Testbestand", "Stock de test", "Stock de prueba"],
  twinStockOk: [
    "With stock: {with}. Without stock: {without}.",
    "Mit Bestand: {with}. Ohne Bestand: {without}.",
    "Avec stock : {with}. Sans stock : {without}.",
    "Con stock: {with}. Sin stock: {without}.",
  ],
  twinStockEmpty: [
    "No item has stock. Imported products start at zero, so nobody can buy yet. Stock you set here stays in this copy.",
    "Kein Artikel hat Bestand. Importierte Produkte starten bei null, deshalb kann noch niemand kaufen. Bestand, den du hier setzt, bleibt in dieser Kopie.",
    "Aucun article n’a de stock. Les produits importés démarrent à zéro, personne ne peut donc acheter. Le stock défini ici reste dans cette copie.",
    "Ningún artículo tiene stock. Los productos importados empiezan en cero, así que nadie puede comprar. El stock que pongas aquí se queda en esta copia.",
  ],
  twinOpenProducts: [
    "Open products",
    "Produkte öffnen",
    "Ouvrir les produits",
    "Abrir productos",
  ],
  twinCheckout: ["Checkout", "Checkout", "Paiement", "Pago"],
  twinCheckoutOk: [
    "Simulated payment and shipping are set up for {country}.",
    "Simulierte Zahlung und Versand sind für {country} eingerichtet.",
    "Paiement simulé et livraison sont configurés pour {country}.",
    "El pago simulado y el envío están configurados para {country}.",
  ],
  twinCheckoutPayment: [
    "No simulated payment method is active for {country}.",
    "Für {country} ist keine simulierte Zahlungsart aktiv.",
    "Aucun mode de paiement simulé n’est actif pour {country}.",
    "No hay ningún método de pago simulado activo para {country}.",
  ],
  twinCheckoutShipping: [
    "No shipping method is active for {country}.",
    "Für {country} ist keine Versandart aktiv.",
    "Aucun mode de livraison n’est actif pour {country}.",
    "No hay ningún método de envío activo para {country}.",
  ],
  twinCheckoutCountry: [
    "No delivery country is enabled.",
    "Es ist kein Lieferland aktiviert.",
    "Aucun pays de livraison n’est activé.",
    "No hay ningún país de entrega activado.",
  ],
  twinOpenSettings: [
    "Open settings",
    "Einstellungen öffnen",
    "Ouvrir les paramètres",
    "Abrir ajustes",
  ],
  twinTests: [
    "Customer tests",
    "Kundentests",
    "Tests clients",
    "Pruebas de clientes",
  ],
  twinTestsOk: [
    "Saved customer tests in this copy: {count}.",
    "Gespeicherte Kundentests in dieser Kopie: {count}.",
    "Tests clients enregistrés dans cette copie : {count}.",
    "Pruebas de clientes guardadas en esta copia: {count}.",
  ],
  twinTestsNone: [
    "No customer test yet. Describe a goal below and let the customers try it.",
    "Noch kein Kundentest. Beschreibe unten ein Ziel und lass die Kunden es ausprobieren.",
    "Aucun test client pour l’instant. Décrivez un objectif ci-dessous et laissez les clients essayer.",
    "Aún no hay pruebas de clientes. Describe un objetivo abajo y deja que los clientes lo prueben.",
  ],
  twinDone: ["Done", "Erledigt", "Fait", "Hecho"],
  twinOpen: ["Open", "Offen", "À faire", "Pendiente"],
  twinFill: [
    "Set test stock to 10",
    "Testbestand auf 10 setzen",
    "Mettre le stock de test à 10",
    "Poner stock de prueba en 10",
  ],
  twinFilled: [
    "Test stock set for {count} items. It stays in this copy.",
    "Testbestand für {count} Artikel gesetzt. Er bleibt in dieser Kopie.",
    "Stock de test défini pour {count} articles. Il reste dans cette copie.",
    "Stock de prueba puesto en {count} artículos. Se queda en esta copia.",
  ],
  twinAttack: [
    "Attack test",
    "Angriffstest",
    "Test d’attaque",
    "Prueba de ataque",
  ],
  twinAttackOk: [
    "Attacks refused: {held} of {total}. The configuration is unchanged since then.",
    "Abgewehrte Angriffe: {held} von {total}. Die Konfiguration ist seitdem unverändert.",
    "Attaques refusées : {held} sur {total}. La configuration n’a pas changé depuis.",
    "Ataques rechazados: {held} de {total}. La configuración no ha cambiado desde entonces.",
  ],
  twinAttackStale: [
    "The last attack test is older than your latest changes. Run it again below.",
    "Der letzte Angriffstest ist älter als deine letzten Änderungen. Starte ihn unten erneut.",
    "Le dernier test d’attaque est antérieur à vos dernières modifications. Relancez-le ci-dessous.",
    "La última prueba de ataque es anterior a tus últimos cambios. Lánzala de nuevo abajo.",
  ],
  twinAttackBreached: [
    "The last attack test found attacks that got through or need a check. See the result below.",
    "Der letzte Angriffstest hat Angriffe gefunden, die durchkamen oder zu prüfen sind. Sieh dir das Ergebnis unten an.",
    "Le dernier test a trouvé des attaques réussies ou à vérifier. Voir le résultat ci-dessous.",
    "La última prueba encontró ataques que pasaron o que hay que revisar. Mira el resultado abajo.",
  ],
  twinAttackNone: [
    "No attack test yet. Run the attacks below.",
    "Noch kein Angriffstest. Starte unten die Angriffe.",
    "Aucun test d’attaque pour l’instant. Lancez les attaques ci-dessous.",
    "Aún no hay prueba de ataque. Lanza los ataques abajo.",
  ],
} as const;
export type TwinWord = keyof typeof words;
export function useTwinText() {
  const { locale } = useLocale();
  const index =
    ({ en: 0, de: 1, fr: 2, es: 3 } as Record<string, number>)[
      locale.slice(0, 2)
    ] ?? 0;
  return (key: TwinWord, values: Record<string, string | number> = {}) =>
    Object.entries(values).reduce<string>(
      (text, [name, value]) => text.replace(`{${name}}`, String(value)),
      words[key][index],
    );
}
