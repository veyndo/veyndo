/** Four-language vocabulary of integration keys: creation, lifetime, rights and revocation. */
import { useLocale } from "../../../../../frontend/src/shared/i18n/i18n";
const words = {
  copy: ["Copy key", "Schlüssel kopieren", "Copier la clé", "Copiar clave"],
  copied: ["Copied", "Kopiert", "Copié", "Copiada"],
  clear: ["Clear key", "Schlüssel verwerfen", "Effacer la clé", "Borrar clave"],
  create: ["Create key", "Schlüssel erstellen", "Créer la clé", "Crear clave"],
  days: [
    "Validity in days (1–90)",
    "Gültigkeit in Tagen (1–90)",
    "Validité en jours (1–90)",
    "Validez en días (1–90)",
  ],
  expired: ["Expired", "Abgelaufen", "Expirée", "Caducada"],
  expiry: ["Expires", "Gültig bis", "Expire", "Caduca"],
  keys: [
    "Integration keys",
    "Integrationsschlüssel",
    "Clés d’intégration",
    "Claves de integración",
  ],
  lifetime: [
    "Key lifetime",
    "Laufzeit des Schlüssels",
    "Durée de la clé",
    "Duración de la clave",
  ],
  limited: ["Time limited", "Zeitlich begrenzt", "Limitée", "Limitada"],
  manageHint: [
    "Managing keys requires team.manage. Scopes are always intersected with the creator’s current membership.",
    "Schlüsselverwaltung benötigt team.manage. Rechte werden immer mit den aktuellen Mitgliedsrechten des Erstellers abgeglichen.",
    "La gestion nécessite team.manage. Les droits sont limités aux droits actuels du créateur.",
    "La gestión requiere team.manage. Los permisos se limitan a los derechos actuales del creador.",
  ],
  name: [
    "Key name",
    "Name des Schlüssels",
    "Nom de la clé",
    "Nombre de la clave",
  ],
  once: [
    "Shown once. Copy securely; closing this view clears the plaintext key.",
    "Einmalige Anzeige. Sicher kopieren; beim Schließen wird der Klartextschlüssel verworfen.",
    "Affichage unique. Copiez en lieu sûr ; fermer efface la clé en clair.",
    "Se muestra una vez. Copia de forma segura; cerrar elimina la clave visible.",
  ],
  permanent: [
    "No expiry",
    "Unbegrenzt gültig",
    "Sans expiration",
    "Sin caducidad",
  ],
  permanentHint: [
    "Personal administrators can create non-expiring keys. Revocation and current membership checks still apply.",
    "Persönliche Administratoren können unbegrenzte Schlüssel erstellen. Widerruf und aktuelle Mitgliedsrechte gelten weiterhin.",
    "Les administrateurs personnels peuvent créer des clés sans expiration. Révocation et droits actuels restent applicables.",
    "Los administradores personales pueden crear claves sin caducidad. Se mantienen la revocación y los permisos actuales.",
  ],
  revoke: [
    "Revoke key",
    "Schlüssel widerrufen",
    "Révoquer la clé",
    "Revocar clave",
  ],
  revokeHint: [
    "Connected integrations will lose access immediately.",
    "Verbundene Integrationen verlieren sofort den Zugriff.",
    "Les intégrations perdent immédiatement l’accès.",
    "Las integraciones perderán acceso de inmediato.",
  ],
  rights: [
    "Granted permissions",
    "Erteilte Rechte",
    "Droits accordés",
    "Permisos concedidos",
  ],
} as const;
export function useIntegrationKeyText() {
  const { locale } = useLocale();
  return (k: keyof typeof words) =>
    words[k][{ "en-GB": 0, "de-DE": 1, "fr-FR": 2, "es-ES": 3 }[locale]];
}
