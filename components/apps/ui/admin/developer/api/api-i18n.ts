/** Four-language developer API console vocabulary; no credentials are persisted in UI storage. */
import { useLocale } from "../../../../../../frontend/src/shared/i18n/i18n";
const words = {
  studio: ["App Studio", "App Studio", "App Studio", "App Studio"],
  console: [
    "API & integrations",
    "API & Integrationen",
    "API et intégrations",
    "API e integraciones",
  ],
  intro: [
    "One Rust backend for storefront, Studio, MCP and apps. Every request is checked for shop and permissions.",
    "Ein Rust-Backend für Storefront, Studio, MCP und Apps. Jede Anfrage wird auf Shop und Berechtigungen geprüft.",
    "Un backend Rust pour la boutique, Studio, MCP et les apps. Chaque requête est contrôlée par boutique et droits.",
    "Un backend Rust para tienda, Studio, MCP y apps. Cada petición comprueba tienda y permisos.",
  ],
  routes: [
    "Endpoint explorer",
    "API-Explorer",
    "Explorateur d’API",
    "Explorador de API",
  ],
  search: [
    "Search API path or method",
    "API-Pfad oder Methode suchen",
    "Rechercher chemin ou méthode",
    "Buscar ruta o método",
  ],
  test: [
    "Run read request",
    "Leseanfrage ausführen",
    "Exécuter la lecture",
    "Ejecutar lectura",
  ],
  key: [
    "Optional integration key for this test",
    "Optionaler Integrationsschlüssel für diesen Test",
    "Clé d’intégration facultative pour le test",
    "Clave de integración opcional para esta prueba",
  ],
  testing: ["Testing…", "Wird getestet…", "Test en cours…", "Probando…"],
  readOnly: [
    "Live testing is limited to read operations. Write routes are documented here and use explicit merchant actions elsewhere.",
    "Live-Tests sind auf Leseoperationen begrenzt. Schreiboperationen sind hier dokumentiert und werden über explizite Händleraktionen ausgeführt.",
    "Les tests sont limités aux lectures. Les écritures sont documentées et exécutées par des actions explicites.",
    "Las pruebas se limitan a lecturas. Las escrituras están documentadas y usan acciones explícitas.",
  ],
  result: ["Response", "Antwort", "Réponse", "Respuesta"],
  scope: [
    "Current shop",
    "Aktueller Shop",
    "Boutique actuelle",
    "Tienda actual",
  ],
  protocol: [
    "MCP discovery",
    "MCP-Erkundung",
    "Découverte MCP",
    "Descubrimiento MCP",
  ],
  apps: [
    "Shop-specific app routes",
    "Shop-spezifische App-Routen",
    "Routes d’apps de la boutique",
    "Rutas de apps de la tienda",
  ],
} as const;
export function useApiText() {
  const { locale } = useLocale();
  return (k: keyof typeof words) =>
    words[k][{ "en-GB": 0, "de-DE": 1, "fr-FR": 2, "es-ES": 3 }[locale]];
}
