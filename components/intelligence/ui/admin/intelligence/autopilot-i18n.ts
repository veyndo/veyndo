/** Autopilot vocabulary: levels per area, daily limits and today's use in EN/DE/FR/ES; amounts are parameters. */
import { useLocale } from "../../../../../frontend/src/shared/i18n/i18n";
const words = {
  title: ["Autopilot", "Autopilot", "Pilote automatique", "Piloto automático"],
  intro: [
    "Decide per area how far an agent may go without asking you.",
    "Lege pro Bereich fest, wie weit ein Agent ohne Rückfrage gehen darf.",
    "Décidez par domaine jusqu’où un agent peut aller sans vous demander.",
    "Decide por área hasta dónde puede llegar un agente sin preguntarte.",
  ],
  level0: [
    "Level 0 · Suggests",
    "Stufe 0 · Schlägt vor",
    "Niveau 0 · Propose",
    "Nivel 0 · Propone",
  ],
  level1: [
    "Level 1 · Acts within the budget",
    "Stufe 1 · Handelt im Budget",
    "Niveau 1 · Agit dans le budget",
    "Nivel 1 · Actúa dentro del presupuesto",
  ],
  areaPrices: ["Prices", "Preise", "Prix", "Precios"],
  areaStock: ["Stock", "Bestand", "Stock", "Stock"],
  areaContent: [
    "Texts and translations",
    "Texte und Übersetzungen",
    "Textes et traductions",
    "Textos y traducciones",
  ],
  areaDesign: [
    "Shop design",
    "Shopdesign",
    "Design de la boutique",
    "Diseño de la tienda",
  ],
  areaRules: [
    "Rules, flows and apps",
    "Regeln, Flows und Apps",
    "Règles, flows et apps",
    "Reglas, flows y apps",
  ],
  fixed: [
    "Outside prices an agent always suggests and you approve.",
    "Außerhalb der Preise schlägt ein Agent immer vor und du gibst frei.",
    "En dehors des prix, un agent propose toujours et vous validez.",
    "Fuera de los precios, un agente siempre propone y tú apruebas.",
  ],
  today: ["Today (UTC)", "Heute (UTC)", "Aujourd’hui (UTC)", "Hoy (UTC)"],
  products: [
    "Products changed: {used} of {limit}",
    "Geänderte Produkte: {used} von {limit}",
    "Produits modifiés : {used} sur {limit}",
    "Productos cambiados: {used} de {limit}",
  ],
  exposure: [
    "Price reductions: {used} of {limit}",
    "Preissenkungen: {used} von {limit}",
    "Baisses de prix : {used} sur {limit}",
    "Bajadas de precio: {used} de {limit}",
  ],
  exposureOpen: [
    "Price reductions: {used}. No daily limit is set.",
    "Preissenkungen: {used}. Es ist kein Tageslimit gesetzt.",
    "Baisses de prix : {used}. Aucune limite quotidienne n’est définie.",
    "Bajadas de precio: {used}. No hay límite diario.",
  ],
  exposureHint: [
    "Reduction times current stock: the most revenue you give up if the whole stock sells at the lower price. Price increases count as zero.",
    "Senkung mal aktuellem Bestand: so viel Umsatz gibst du höchstens auf, wenn der ganze Bestand zum gesenkten Preis verkauft wird. Preiserhöhungen zählen null.",
    "Baisse multipliée par le stock actuel : le chiffre d’affaires maximal abandonné si tout le stock se vend au prix réduit. Les hausses comptent pour zéro.",
    "Bajada por stock actual: el máximo ingreso que cedes si todo el stock se vende al precio rebajado. Las subidas cuentan cero.",
  ],
  none: [
    "The autopilot has not changed a price today.",
    "Der Autopilot hat heute keinen Preis geändert.",
    "Le pilote automatique n’a modifié aucun prix aujourd’hui.",
    "El piloto automático no ha cambiado ningún precio hoy.",
  ],
  change: [
    "{from} → {to} · stock {stock} · counts {amount}",
    "{from} → {to} · Bestand {stock} · zählt {amount}",
    "{from} → {to} · stock {stock} · compte {amount}",
    "{from} → {to} · stock {stock} · cuenta {amount}",
  ],
  otherCurrency: [
    "Other currency, not counted",
    "Andere Währung, nicht gezählt",
    "Autre devise, non comptée",
    "Otra moneda, no contada",
  ],
  limitLabel: [
    "Daily limit for price reductions · {currency}",
    "Tageslimit für Preissenkungen · {currency}",
    "Limite quotidienne des baisses de prix · {currency}",
    "Límite diario de bajadas de precio · {currency}",
  ],
  limitHint: [
    "Leave empty for no limit. Once the limit is reached, the autopilot refuses further reductions today.",
    "Leer lassen für kein Limit. Ist das Limit erreicht, lehnt der Autopilot heute weitere Senkungen ab.",
    "Laissez vide pour aucune limite. Une fois la limite atteinte, le pilote automatique refuse d’autres baisses aujourd’hui.",
    "Déjalo vacío para no tener límite. Al alcanzarlo, el piloto automático rechaza más bajadas hoy.",
  ],
  loading: [
    "Reading today’s use…",
    "Heutiger Verbrauch wird gelesen…",
    "Lecture de l’utilisation du jour…",
    "Leyendo el uso de hoy…",
  ],
  revert: ["Take back", "Zurücknehmen", "Annuler", "Revertir"],
  revertAll: [
    "Take back all of today’s changes",
    "Alle heutigen Änderungen zurücknehmen",
    "Annuler toutes les modifications du jour",
    "Revertir todos los cambios de hoy",
  ],
  reverted: [
    "Restored to the price at the start of the day: {count}.",
    "Auf den Preis vom Tagesbeginn zurückgesetzt: {count}.",
    "Rétablis au prix du début de journée : {count}.",
    "Restaurados al precio del inicio del día: {count}.",
  ],
  revertSkipped: [
    "Not restored because the price was changed by hand afterwards: {count}.",
    "Nicht zurückgesetzt, weil der Preis danach von Hand geändert wurde: {count}.",
    "Non rétablis car le prix a ensuite été modifié à la main : {count}.",
    "No restaurados porque el precio se cambió a mano después: {count}.",
  ],
  atBaseline: [
    "Back at the price from the start of the day",
    "Wieder auf dem Preis vom Tagesbeginn",
    "De nouveau au prix du début de journée",
    "De nuevo al precio del inicio del día",
  ],
  guardHint: [
    "If today’s total rises above the limit, for example because stock arrives or you lower the limit, the autopilot takes today’s reductions back by itself within about a minute.",
    "Steigt die heutige Summe über das Limit, zum Beispiel weil Ware eintrifft oder du das Limit senkst, nimmt der Autopilot die heutigen Senkungen innerhalb etwa einer Minute von selbst zurück.",
    "Si le total du jour dépasse la limite, par exemple à l’arrivée de stock ou si vous baissez la limite, le pilote automatique annule de lui-même les baisses du jour en une minute environ.",
    "Si el total de hoy supera el límite, por ejemplo al llegar stock o si bajas el límite, el piloto automático revierte por sí mismo las bajadas de hoy en un minuto aproximadamente.",
  ],
} as const;
export type AutopilotWord = keyof typeof words;
export function useAutopilotText() {
  const { locale } = useLocale();
  const index =
    ({ en: 0, de: 1, fr: 2, es: 3 } as Record<string, number>)[
      locale.slice(0, 2)
    ] ?? 0;
  return (key: AutopilotWord, values: Record<string, string | number> = {}) =>
    Object.entries(values).reduce<string>(
      (text, [name, value]) => text.replace(`{${name}}`, String(value)),
      words[key][index],
    );
}
