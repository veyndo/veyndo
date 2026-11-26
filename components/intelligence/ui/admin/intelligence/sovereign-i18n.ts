/** Sovereign AI vocabulary: the switch state, one line per model route and the limits of the statement in EN/DE/FR/ES. */
import { useLocale } from "../../../../../frontend/src/shared/i18n/i18n";
const words = {
  title: [
    "Sovereign AI mode",
    "Souveräner KI-Betrieb",
    "Mode IA souveraine",
    "Modo de IA soberana",
  ],
  on: [
    "On: model requests only go to self-hosted models.",
    "An: Modellanfragen gehen nur an selbst betriebene Modelle.",
    "Activé : les requêtes ne vont qu’à des modèles auto-hébergés.",
    "Activado: las peticiones solo van a modelos autoalojados.",
  ],
  onBroken: [
    "On, but one route is not self-hosted. Veyndo refuses requests on that route.",
    "An, aber eine Strecke ist nicht selbst betrieben. Veyndo lehnt Anfragen auf dieser Strecke ab.",
    "Activé, mais une route n’est pas auto-hébergée. Veyndo refuse les requêtes sur cette route.",
    "Activado, pero una ruta no es autoalojada. Veyndo rechaza las peticiones en esa ruta.",
  ],
  off: [
    "Off: hosted model providers are allowed. The platform operator switches this mode on.",
    "Aus: gehostete Modellanbieter sind erlaubt. Der Plattformbetreiber schaltet diesen Betrieb ein.",
    "Désactivé : les fournisseurs hébergés sont autorisés. L’opérateur de la plateforme active ce mode.",
    "Desactivado: se permiten proveedores alojados. El operador de la plataforma activa este modo.",
  ],
  locked: [
    "Fixed by the deployment; it cannot be switched off in Studio.",
    "Durch die Installation festgelegt; im Studio nicht abschaltbar.",
    "Fixé par le déploiement ; impossible à désactiver dans Studio.",
    "Fijado por el despliegue; no se puede desactivar en Studio.",
  ],
  text: [
    "Chat, product questions, translations",
    "Chat, Produktfragen, Übersetzungen",
    "Chat, questions produit, traductions",
    "Chat, preguntas de producto, traducciones",
  ],
  embeddings: [
    "Search index (embeddings)",
    "Suchindex (Embeddings)",
    "Index de recherche (embeddings)",
    "Índice de búsqueda (embeddings)",
  ],
  reranker: [
    "Search ranking",
    "Suchreihenfolge",
    "Classement de recherche",
    "Orden de búsqueda",
  ],
  images: [
    "Image generation",
    "Bilderzeugung",
    "Génération d’images",
    "Generación de imágenes",
  ],
  own: [
    "{host} · self-hosted",
    "{host} · selbst betrieben",
    "{host} · auto-hébergé",
    "{host} · autoalojado",
  ],
  foreign: [
    "{host} · external",
    "{host} · extern",
    "{host} · externe",
    "{host} · externo",
  ],
  refused: [
    "{host} · external, refused",
    "{host} · extern, abgelehnt",
    "{host} · externe, refusé",
    "{host} · externo, rechazado",
  ],
  unused: [
    "Not set up",
    "Nicht eingerichtet",
    "Non configuré",
    "Sin configurar",
  ],
  imagesRefused: [
    "Refused: uses a hosted provider",
    "Abgelehnt: nutzt einen gehosteten Anbieter",
    "Refusé : utilise un fournisseur hébergé",
    "Rechazado: usa un proveedor alojado",
  ],
  providersRefused: [
    "Refused providers: {list}",
    "Abgelehnte Anbieter: {list}",
    "Fournisseurs refusés : {list}",
    "Proveedores rechazados: {list}",
  ],
  limits: [
    "Veyndo judges a host by its network range and by the hosts the deployment declares as its own; it does not resolve DNS. Apps you install are separate services with their own connections.",
    "Veyndo beurteilt einen Host nach seinem Netzbereich und nach den Hosts, die die Installation als eigene angibt; DNS wird nicht aufgelöst. Installierte Apps sind eigene Dienste mit eigenen Verbindungen.",
    "Veyndo juge un hôte d’après sa plage réseau et les hôtes que le déploiement déclare comme siens ; le DNS n’est pas résolu. Les apps installées sont des services séparés avec leurs propres connexions.",
    "Veyndo juzga un host por su rango de red y por los hosts que el despliegue declara como propios; no resuelve DNS. Las apps instaladas son servicios separados con sus propias conexiones.",
  ],
} as const;
export type SovereignWord = keyof typeof words;
export function useSovereignText() {
  const { locale } = useLocale();
  const index =
    ({ en: 0, de: 1, fr: 2, es: 3 } as Record<string, number>)[
      locale.slice(0, 2)
    ] ?? 0;
  return (key: SovereignWord, values: Record<string, string | number> = {}) =>
    Object.entries(values).reduce<string>(
      (text, [name, value]) => text.replace(`{${name}}`, String(value)),
      words[key][index],
    );
}
