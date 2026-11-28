/** Review vocabulary for typed operation facts: risk, reversibility, approval, affected data and side effects. */
import { useLocale } from "../i18n/i18n";
const words = {
  facts: [
    "What this step does",
    "Was dieser Schritt bewirkt",
    "Effet de cette étape",
    "Qué hace este paso",
  ],
  risk_low: ["Low risk", "Geringes Risiko", "Risque faible", "Riesgo bajo"],
  risk_medium: [
    "Medium risk",
    "Mittleres Risiko",
    "Risque moyen",
    "Riesgo medio",
  ],
  risk_high: ["High risk", "Hohes Risiko", "Risque élevé", "Riesgo alto"],
  approval: [
    "Needs your approval",
    "Braucht deine Freigabe",
    "Nécessite votre validation",
    "Requiere tu aprobación",
  ],
  reversible: [
    "Can be undone",
    "Rückgängig machbar",
    "Annulable",
    "Se puede deshacer",
  ],
  compensatable: [
    "Undo with {operation}",
    "Rückgängig mit {operation}",
    "Annulable avec {operation}",
    "Se deshace con {operation}",
  ],
  irreversible: [
    "Cannot be undone",
    "Nicht rückgängig machbar",
    "Irréversible",
    "No se puede deshacer",
  ],
  readOnly: ["Only reads", "Liest nur", "Lecture seule", "Solo lectura"],
  pii: [
    "Personal data",
    "Personenbezogene Daten",
    "Données personnelles",
    "Datos personales",
  ],
  money: ["Money", "Geld", "Argent", "Dinero"],
  stock: ["Stock", "Bestand", "Stock", "Existencias"],
  email: [
    "Sends email",
    "Sendet E-Mails",
    "Envoie des e-mails",
    "Envía correos",
  ],
  payment: [
    "Moves money",
    "Bewegt Geld",
    "Déplace de l’argent",
    "Mueve dinero",
  ],
  externalCall: [
    "Calls an external service",
    "Ruft einen externen Dienst auf",
    "Appelle un service externe",
    "Llama a un servicio externo",
  ],
  modelCall: [
    "Uses the AI model",
    "Nutzt das KI-Modell",
    "Utilise le modèle IA",
    "Usa el modelo de IA",
  ],
  publication: [
    "Visible to shoppers",
    "Für Kundschaft sichtbar",
    "Visible par les clients",
    "Visible para la clientela",
  ],
  backgroundJob: [
    "Runs in the background",
    "Läuft im Hintergrund",
    "S’exécute en arrière-plan",
    "Se ejecuta en segundo plano",
  ],
  event: [
    "Can trigger Flows and apps",
    "Kann Flows und Apps auslösen",
    "Peut déclencher des Flows et des apps",
    "Puede activar Flows y apps",
  ],
  sandbox: [
    "Private sandbox only",
    "Nur in einer privaten Sandbox",
    "Bac à sable privé uniquement",
    "Solo en un entorno de pruebas privado",
  ],
  personalSession: [
    "Personal login only",
    "Nur mit persönlichem Login",
    "Connexion personnelle uniquement",
    "Solo con inicio de sesión personal",
  ],
  permissions: [
    "Requires: {permissions}",
    "Benötigt: {permissions}",
    "Nécessite : {permissions}",
    "Requiere: {permissions}",
  ],
  unknown: [
    "No declared facts; review it like an irreversible change",
    "Keine deklarierten Angaben; prüfe den Schritt wie eine endgültige Änderung",
    "Aucune information déclarée ; vérifiez-la comme une modification irréversible",
    "Sin datos declarados; revísalo como un cambio irreversible",
  ],
} as const;
export type OperationFactsWord = keyof typeof words;
export function useOperationFactsText() {
  const { locale } = useLocale();
  const i = { "en-GB": 0, "de-DE": 1, "fr-FR": 2, "es-ES": 3 }[locale] ?? 0;
  return (key: OperationFactsWord, values: Record<string, string> = {}) =>
    words[key][i].replace(/\{(\w+)\}/g, (_, name) => values[name] ?? "");
}
