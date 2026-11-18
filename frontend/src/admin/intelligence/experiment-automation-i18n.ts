/** Vocabulary for experiment suggestions and automation in EN/DE/FR/ES; figures are parameters. */
import { useLocale } from "../../shared/i18n/i18n";
const words = {
  suggestTitle: [
    "Suggest an experiment",
    "Experiment vorschlagen",
    "Proposer une expérience",
    "Proponer un experimento",
  ],
  suggestIntro: [
    "Prefill the form from your shop's live payments of the last 30 days. Nothing is saved until you register the experiment.",
    "Füllt das Formular aus den echten Zahlungen deines Shops der letzten 30 Tage vor. Gespeichert wird erst, wenn du das Experiment registrierst.",
    "Préremplit le formulaire à partir des paiements réels des 30 derniers jours. Rien n’est enregistré avant que vous n’enregistriez l’expérience.",
    "Rellena el formulario con los pagos reales de los últimos 30 días. No se guarda nada hasta que registres el experimento.",
  ],
  suggestButton: [
    "Suggest from shop figures",
    "Aus Shop-Zahlen vorschlagen",
    "Proposer à partir des chiffres",
    "Proponer con las cifras de la tienda",
  ],
  suggestBasis: [
    "Basis: {count} live payments in 30 days, 95 % of them up to {amount}. Current layout mode: {mode}.",
    "Grundlage: {count} echte Zahlungen in 30 Tagen, 95 % davon bis {amount}. Aktueller Layout-Modus: {mode}.",
    "Base : {count} paiements réels en 30 jours, 95 % jusqu’à {amount}. Mode de mise en page actuel : {mode}.",
    "Base: {count} pagos reales en 30 días, el 95 % hasta {amount}. Modo de diseño actual: {mode}.",
  ],
  suggestUsed: [
    "The form below is prefilled. Check it, then register.",
    "Das Formular unten ist vorausgefüllt. Prüfe es und registriere dann.",
    "Le formulaire ci-dessous est prérempli. Vérifiez-le, puis enregistrez.",
    "El formulario de abajo está relleno. Revísalo y regístralo.",
  ],
  suggestedName: [
    "Layout comparison",
    "Layout-Vergleich",
    "Comparaison de mises en page",
    "Comparación de diseños",
  ],
  warnNoLivePayments: [
    "Your shop has no live payments in the last 30 days. An experiment would run, but its readout would stay empty because only real payments count.",
    "Dein Shop hat in den letzten 30 Tagen keine echten Zahlungen. Ein Experiment würde laufen, seine Auswertung bliebe aber leer, weil nur echte Zahlungen zählen.",
    "Votre boutique n’a aucun paiement réel sur 30 jours. L’expérience tournerait, mais son résultat resterait vide car seuls les paiements réels comptent.",
    "Tu tienda no tiene pagos reales en los últimos 30 días. El experimento se ejecutaría, pero su resultado quedaría vacío porque solo cuentan los pagos reales.",
  ],
  warnLowVolume: [
    "Fewer than 200 live payments in 30 days. Each layout needs at least 100 participants, so the result may stay undecided.",
    "Weniger als 200 echte Zahlungen in 30 Tagen. Jedes Layout braucht mindestens 100 Teilnehmer, das Ergebnis kann daher offen bleiben.",
    "Moins de 200 paiements réels en 30 jours. Chaque mise en page exige au moins 100 participants ; le résultat peut rester indécis.",
    "Menos de 200 pagos reales en 30 días. Cada diseño necesita al menos 100 participantes, así que el resultado puede quedar sin decidir.",
  ],
  warnAlreadyRunning: [
    "An experiment is already running in this sales channel. A second one can only start after it ends.",
    "In diesem Verkaufskanal läuft bereits ein Experiment. Ein zweites kann erst danach starten.",
    "Une expérience est déjà en cours dans ce canal. Une seconde ne peut démarrer qu’après.",
    "Ya hay un experimento en marcha en este canal. Otro solo puede empezar después.",
  ],
  autoFinish: [
    "Finish and read out by itself",
    "Von selbst abschließen und auswerten",
    "Clôturer et analyser automatiquement",
    "Finalizar y evaluar por sí solo",
  ],
  autoFinishHint: [
    "After you start it, the experiment ends at its horizon and the final readout is saved after the settlement period, without further clicks.",
    "Nach deinem Start endet das Experiment zum festgelegten Zeitpunkt, und die Endauswertung wird nach der Abrechnungsfrist gespeichert, ohne weitere Klicks.",
    "Après votre démarrage, l’expérience se termine à l’échéance et le résultat final est enregistré après le délai de règlement, sans autre clic.",
    "Tras iniciarlo, el experimento termina en su plazo y el resultado final se guarda tras el periodo de liquidación, sin más clics.",
  ],
  autoApply: [
    "Adopt a proved layout by itself",
    "Bewiesenes Layout von selbst übernehmen",
    "Adopter automatiquement une mise en page prouvée",
    "Adoptar por sí solo un diseño demostrado",
  ],
  autoApplyHint: [
    "Only if the final readout proves an uplift, the tested layout becomes the fixed layout of the shop. Otherwise the shop stays as it is.",
    "Nur wenn die Endauswertung einen Zuwachs beweist, wird das getestete Layout zum festen Layout des Shops. Sonst bleibt der Shop, wie er ist.",
    "Seulement si le résultat final prouve un gain, la mise en page testée devient la mise en page fixe de la boutique. Sinon rien ne change.",
    "Solo si el resultado final demuestra una mejora, el diseño probado pasa a ser el diseño fijo de la tienda. Si no, la tienda queda como está.",
  ],
  badgeAuto: [
    "Runs by itself",
    "Läuft von selbst",
    "Automatique",
    "Funciona solo",
  ],
  badgeApply: [
    "Adopts a proved layout",
    "Übernimmt bewiesenes Layout",
    "Adopte une mise en page prouvée",
    "Adopta un diseño demostrado",
  ],
  decisionPublished: [
    "Decided by itself: the tested layout was adopted.",
    "Von selbst entschieden: Das getestete Layout wurde übernommen.",
    "Décidé automatiquement : la mise en page testée a été adoptée.",
    "Decidido por sí solo: se adoptó el diseño probado.",
  ],
  decisionKept: [
    "Decided by itself: no proved uplift, the shop stays as it was.",
    "Von selbst entschieden: kein bewiesener Zuwachs, der Shop bleibt, wie er war.",
    "Décidé automatiquement : aucun gain prouvé, la boutique reste inchangée.",
    "Decidido por sí solo: sin mejora demostrada, la tienda queda como estaba.",
  ],
} as const;
export type ExperimentAutomationWord = keyof typeof words;
export function useExperimentAutomationText() {
  const { locale } = useLocale();
  const index =
    ({ en: 0, de: 1, fr: 2, es: 3 } as Record<string, number>)[
      locale.slice(0, 2)
    ] ?? 0;
  return (
    key: ExperimentAutomationWord,
    values: Record<string, string | number> = {},
  ) =>
    Object.entries(values).reduce<string>(
      (text, [name, value]) => text.replace(`{${name}}`, String(value)),
      words[key][index],
    );
}
