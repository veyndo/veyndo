/** App sandbox transparency and version vocabulary: what an app may do, what it did, and explicit rollback; EN/DE/FR/ES. */
import { useLocale } from "../../../../../frontend/src/shared/i18n/i18n";
const words = {
  title: [
    "Sandbox and permissions",
    "Sandbox und Berechtigungen",
    "Bac à sable et autorisations",
    "Entorno aislado y permisos",
  ],
  intro: [
    "What this app may do, declared in its signed package, and what it actually did, from the shop's own call and change records.",
    "Was diese App laut ihrem signierten Paket darf und was sie tatsächlich getan hat, aus den Aufruf- und Änderungsprotokollen des Shops.",
    "Ce que cette app peut faire selon son paquet signé, et ce qu’elle a réellement fait, d’après les journaux d’appels et de modifications de la boutique.",
    "Lo que esta app puede hacer según su paquete firmado y lo que realmente hizo, según los registros de llamadas y cambios de la tienda.",
  ],
  mayDo: ["May do", "Darf", "Peut faire", "Puede hacer"],
  didDo: ["Did", "Hat getan", "A fait", "Hizo"],
  permissions: ["Permissions", "Berechtigungen", "Autorisations", "Permisos"],
  noPermissions: [
    "No core permissions",
    "Keine Kernberechtigungen",
    "Aucune autorisation principale",
    "Sin permisos principales",
  ],
  egress: [
    "Network access",
    "Netzwerkzugriff",
    "Accès réseau",
    "Acceso a la red",
  ],
  noEgress: [
    "No network access: the app runs only inside the shop's bounded runtime.",
    "Kein Netzwerkzugriff: Die App läuft nur in der begrenzten Laufzeit des Shops.",
    "Aucun accès réseau : l’app ne s’exécute que dans l’environnement borné de la boutique.",
    "Sin acceso a la red: la app solo se ejecuta en el entorno limitado de la tienda.",
  ],
  service: [
    "Calls its approved service",
    "Ruft ihren freigegebenen Dienst auf",
    "Appelle son service approuvé",
    "Llama a su servicio aprobado",
  ],
  notApproved: [
    "Service not approved by the operator",
    "Dienst vom Betreiber nicht freigegeben",
    "Service non approuvé par l’opérateur",
    "Servicio no aprobado por el operador",
  ],
  dataScopes: [
    "Data it keeps",
    "Daten, die sie speichert",
    "Données conservées",
    "Datos que guarda",
  ],
  fields: ["fields", "Felder", "champs", "campos"],
  publicModel: ["public", "öffentlich", "public", "público"],
  privateModel: ["private", "privat", "privé", "privado"],
  ports: ["Ports", "Ports", "Ports", "Puertos"],
  provides: ["Provides", "Stellt bereit", "Fournit", "Ofrece"],
  consumes: ["Uses", "Nutzt", "Utilise", "Usa"],
  unwired: [
    "no provider installed",
    "kein Anbieter installiert",
    "aucun fournisseur installé",
    "ningún proveedor instalado",
  ],
  wiredTo: ["from", "von", "de", "de"],
  tools: [
    "Agent tools",
    "Agenten-Werkzeuge",
    "Outils pour agents",
    "Herramientas para agentes",
  ],
  approval: [
    "approval required",
    "Freigabe nötig",
    "approbation requise",
    "requiere aprobación",
  ],
  noApproval: [
    "no approval",
    "ohne Freigabe",
    "sans approbation",
    "sin aprobación",
  ],
  risk_low: ["low risk", "geringes Risiko", "risque faible", "riesgo bajo"],
  risk_medium: [
    "medium risk",
    "mittleres Risiko",
    "risque moyen",
    "riesgo medio",
  ],
  risk_high: ["high risk", "hohes Risiko", "risque élevé", "riesgo alto"],
  rev_reversible: ["reversible", "umkehrbar", "réversible", "reversible"],
  rev_compensatable: [
    "can be offset",
    "ausgleichbar",
    "compensable",
    "compensable",
  ],
  rev_irreversible: [
    "irreversible",
    "nicht umkehrbar",
    "irréversible",
    "irreversible",
  ],
  defaults: [
    "Not described by the publisher: treated as high risk with approval required.",
    "Vom Herausgeber nicht beschrieben: gilt als hohes Risiko mit Freigabepflicht.",
    "Non décrit par l’éditeur : traité comme risque élevé avec approbation requise.",
    "No descrito por el editor: se trata como riesgo alto con aprobación obligatoria.",
  ],
  calls: [
    "Recent calls",
    "Letzte Aufrufe",
    "Appels récents",
    "Llamadas recientes",
  ],
  egressCalls: [
    "Recent service calls",
    "Letzte Dienstaufrufe",
    "Appels de service récents",
    "Llamadas de servicio recientes",
  ],
  writes: [
    "Recent data changes",
    "Letzte Datenänderungen",
    "Modifications récentes",
    "Cambios recientes",
  ],
  nothing: [
    "Nothing recorded yet",
    "Noch nichts aufgezeichnet",
    "Rien d’enregistré",
    "Nada registrado todavía",
  ],
  signed: ["Signed by", "Signiert von", "Signé par", "Firmado por"],
  unsigned: [
    "Unsigned shop app",
    "Unsignierte Shop-App",
    "App de boutique non signée",
    "App de tienda sin firmar",
  ],
  versions: ["Versions", "Versionen", "Versions", "Versiones"],
  current: ["installed", "installiert", "installée", "instalada"],
  rollback: [
    "Roll back to this version",
    "Auf diese Version zurücksetzen",
    "Revenir à cette version",
    "Volver a esta versión",
  ],
  rollbackTitle: [
    "Roll back the app",
    "App zurücksetzen",
    "Revenir à une version antérieure",
    "Volver a una versión anterior",
  ],
  rollbackWarning: [
    "The stored version {version} is installed again and its app data is converted with the steps the current version declares. Orders, payments and messages already sent are not undone.",
    "Die gespeicherte Version {version} wird erneut installiert und ihre App-Daten werden mit den Schritten der aktuellen Version umgewandelt. Bestellungen, Zahlungen und gesendete Nachrichten werden nicht rückgängig gemacht.",
    "La version enregistrée {version} est réinstallée et ses données sont converties avec les étapes déclarées par la version actuelle. Les commandes, paiements et messages envoyés ne sont pas annulés.",
    "Se reinstala la versión guardada {version} y sus datos se convierten con los pasos que declara la versión actual. Los pedidos, pagos y mensajes ya enviados no se deshacen.",
  ],
  noRollback: [
    "The installed version declares no rollback to this version.",
    "Die installierte Version sieht kein Zurücksetzen auf diese Version vor.",
    "La version installée ne prévoit pas de retour à cette version.",
    "La versión instalada no prevé volver a esta versión.",
  ],
  rolledBack: [
    "Version {version} is installed again.",
    "Version {version} ist wieder installiert.",
    "La version {version} est réinstallée.",
    "La versión {version} vuelve a estar instalada.",
  ],
} as const;
export type SandboxWord = keyof typeof words;
export function useSandboxText() {
  const { locale } = useLocale();
  const index = locale.startsWith("de")
    ? 1
    : locale.startsWith("fr")
      ? 2
      : locale.startsWith("es")
        ? 3
        : 0;
  return (key: SandboxWord, values: Record<string, string> = {}) =>
    Object.entries(values).reduce<string>(
      (text, [name, value]) => text.replaceAll(`{${name}}`, value),
      words[key][index],
    );
}
