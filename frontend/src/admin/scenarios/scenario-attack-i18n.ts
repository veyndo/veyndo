/** Attack test vocabulary: closed probe titles and finding texts in EN/DE/FR/ES; native refusal details stay verbatim. */
import { useLocale } from "../../shared/i18n/i18n";
const words = {
  attackTitle: [
    "Attack test",
    "Angriffstest",
    "Test d’attaque",
    "Prueba de ataque",
  ],
  attackIntro: [
    "Nine scripted attackers try to cheat the cart and checkout of this private sandbox. No model is used and the live shop is not touched.",
    "Neun feste Angreifer versuchen, Warenkorb und Checkout dieser privaten Sandbox auszutricksen. Es wird kein Modell verwendet und der Live-Shop bleibt unberührt.",
    "Neuf attaquants scriptés tentent de tromper le panier et le paiement de cette sandbox privée. Aucun modèle n’est utilisé et la boutique en ligne reste intacte.",
    "Nueve atacantes guionizados intentan engañar al carrito y al pago de esta sandbox privada. No se usa ningún modelo y la tienda en vivo no se toca.",
  ],
  attackStart: [
    "Run attacks",
    "Angriffe starten",
    "Lancer les attaques",
    "Lanzar ataques",
  ],
  attackRunning: [
    "Attacking…",
    "Angriff läuft…",
    "Attaque en cours…",
    "Atacando…",
  ],
  attackSummary: [
    "{held} of {total} attacks were refused.",
    "{held} von {total} Angriffen wurden abgewehrt.",
    "{held} attaques sur {total} ont été refusées.",
    "Se rechazaron {held} de {total} ataques.",
  ],
  attackProduct: [
    "Tested with {product}. One simulated order was placed in this sandbox.",
    "Getestet mit {product}. In dieser Sandbox wurde eine simulierte Bestellung angelegt.",
    "Testé avec {product}. Une commande simulée a été créée dans cette sandbox.",
    "Probado con {product}. Se creó un pedido simulado en esta sandbox.",
  ],
  attackHeld: ["Refused", "Abgewehrt", "Refusée", "Rechazado"],
  attackBreached: ["Got through", "Durchgekommen", "Réussie", "Tuvo éxito"],
  attackReview: ["Check", "Prüfen", "À vérifier", "Revisar"],
  attackRefusal: [
    "The shop answered:",
    "Der Shop antwortete:",
    "La boutique a répondu :",
    "La tienda respondió:",
  ],
  attackNegativeQuantity: [
    "Put a negative quantity in the cart",
    "Negative Menge in den Warenkorb legen",
    "Mettre une quantité négative au panier",
    "Poner una cantidad negativa en el carrito",
  ],
  attackUnknownProduct: [
    "Add a product that does not exist",
    "Ein Produkt hinzufügen, das es nicht gibt",
    "Ajouter un produit inexistant",
    "Añadir un producto que no existe",
  ],
  attackExcessQuantity: [
    "Order more than is in stock",
    "Mehr bestellen, als auf Lager ist",
    "Commander plus que le stock",
    "Pedir más de lo que hay en stock",
  ],
  attackStaleCart: [
    "Change the cart with an outdated version",
    "Warenkorb mit veraltetem Stand ändern",
    "Modifier le panier avec une version périmée",
    "Cambiar el carrito con una versión obsoleta",
  ],
  attackPriceInjection: [
    "Send an own price with the cart item",
    "Eigenen Preis mit der Position mitschicken",
    "Envoyer son propre prix avec l’article",
    "Enviar un precio propio con el artículo",
  ],
  attackGuessedCart: [
    "Open a cart with a guessed key",
    "Warenkorb mit geratenem Schlüssel öffnen",
    "Ouvrir un panier avec une clé devinée",
    "Abrir un carrito con una clave adivinada",
  ],
  attackForgedTotal: [
    "Confirm the order with a forged total",
    "Bestellung mit gefälschter Summe bestätigen",
    "Confirmer la commande avec un total falsifié",
    "Confirmar el pedido con un total falsificado",
  ],
  attackStaleReview: [
    "Confirm an older cart version than the one reviewed",
    "Älteren Warenkorbstand bestätigen als geprüft",
    "Confirmer une version du panier plus ancienne que celle vérifiée",
    "Confirmar una versión del carrito anterior a la revisada",
  ],
  attackReplayedOrder: [
    "Send the same order twice",
    "Dieselbe Bestellung zweimal senden",
    "Envoyer deux fois la même commande",
    "Enviar el mismo pedido dos veces",
  ],
  findingAccepted: [
    "The shop accepted the attack.",
    "Der Shop hat den Angriff angenommen.",
    "La boutique a accepté l’attaque.",
    "La tienda aceptó el ataque.",
  ],
  findingLimited: [
    "The shop limited the quantity to the stock.",
    "Der Shop hat die Menge auf den Bestand begrenzt.",
    "La boutique a limité la quantité au stock.",
    "La tienda limitó la cantidad al stock.",
  ],
  findingRefusedAtCheckout: [
    "The cart held the quantity, but the shop refused to sell it at checkout.",
    "Der Warenkorb hat die Menge aufgenommen, aber der Shop hat den Verkauf im Checkout abgelehnt.",
    "Le panier a accepté la quantité, mais la boutique a refusé la vente au paiement.",
    "El carrito aceptó la cantidad, pero la tienda rechazó la venta en el pago.",
  ],
  findingOverStock: [
    "The shop sold more than the current stock. Check whether backorders are intended.",
    "Der Shop hat mehr als den aktuellen Bestand verkauft. Prüfe, ob Nachlieferungen gewollt sind.",
    "La boutique a vendu plus que le stock actuel. Vérifiez si les reliquats sont voulus.",
    "La tienda vendió más que el stock actual. Comprueba si los pedidos pendientes son intencionados.",
  ],
  findingIgnored: [
    "The shop ignored the sent price and charged its own.",
    "Der Shop hat den mitgeschickten Preis ignoriert und den eigenen berechnet.",
    "La boutique a ignoré le prix envoyé et appliqué le sien.",
    "La tienda ignoró el precio enviado y cobró el suyo.",
  ],
  findingTotalChanged: [
    "The sent price changed the total.",
    "Der mitgeschickte Preis hat die Summe verändert.",
    "Le prix envoyé a modifié le total.",
    "El precio enviado cambió el total.",
  ],
  findingSameOrder: [
    "Both requests returned the same order and stock fell once.",
    "Beide Anfragen lieferten dieselbe Bestellung und der Bestand sank einmal.",
    "Les deux requêtes ont renvoyé la même commande et le stock a baissé une fois.",
    "Ambas solicitudes devolvieron el mismo pedido y el stock bajó una vez.",
  ],
  findingReplayRefused: [
    "The first order was placed and the repeat was refused.",
    "Die erste Bestellung wurde angelegt und die Wiederholung abgelehnt.",
    "La première commande a été créée et la répétition refusée.",
    "Se creó el primer pedido y la repetición se rechazó.",
  ],
  findingDuplicateOrder: [
    "The repeat created a second order.",
    "Die Wiederholung hat eine zweite Bestellung erzeugt.",
    "La répétition a créé une deuxième commande.",
    "La repetición creó un segundo pedido.",
  ],
  findingStockDouble: [
    "Stock fell more than once for one order.",
    "Der Bestand sank für eine Bestellung mehr als einmal.",
    "Le stock a baissé plus d’une fois pour une commande.",
    "El stock bajó más de una vez por un pedido.",
  ],
  findingCheckoutUnavailable: [
    "The honest checkout could not be prepared, so this attack was not tried.",
    "Der ehrliche Checkout ließ sich nicht vorbereiten, daher wurde dieser Angriff nicht versucht.",
    "Le paiement honnête n’a pas pu être préparé ; cette attaque n’a pas été tentée.",
    "No se pudo preparar el pago honesto, así que este ataque no se intentó.",
  ],
  findingFailedUnexpectedly: [
    "The shop failed with a server error instead of refusing cleanly.",
    "Der Shop ist mit einem Serverfehler ausgefallen, statt sauber abzulehnen.",
    "La boutique a échoué avec une erreur serveur au lieu de refuser proprement.",
    "La tienda falló con un error de servidor en lugar de rechazar limpiamente.",
  ],
} as const;
export type AttackWord = keyof typeof words;
export function useAttackText() {
  const { locale } = useLocale();
  const index =
    ({ en: 0, de: 1, fr: 2, es: 3 } as Record<string, number>)[
      locale.slice(0, 2)
    ] ?? 0;
  return (key: AttackWord) => words[key][index];
}
