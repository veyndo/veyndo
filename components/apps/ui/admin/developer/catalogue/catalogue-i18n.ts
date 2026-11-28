/** Vocabulary of the developer workspace's component tab in four languages; the gallery brings its own from kit/catalog. */
import { useLocale } from "../../../../../../frontend/src/shared/i18n/i18n";
const words = {
  tab: ["Components", "Komponenten", "Composants", "Componentes"],
} as const;
export type CatalogueWord = keyof typeof words;
export function useCatalogueText() {
  const { locale } = useLocale();
  return (key: CatalogueWord) =>
    words[key][{ "en-GB": 0, "de-DE": 1, "fr-FR": 2, "es-ES": 3 }[locale]];
}
