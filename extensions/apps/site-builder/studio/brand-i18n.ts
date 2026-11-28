/** Interface vocabulary of brand themes and motion in the shop designer (English, German, French, Spanish). */
import { useLocale } from "../../../../frontend/src/shared/i18n/i18n";
const words = {
  createBrand: [
    "Create theme from brand",
    "Theme aus Marke erstellen",
    "Créer un thème depuis la marque",
    "Crear tema desde la marca",
  ],
  brandTitle: [
    "Theme from your brand",
    "Theme aus deiner Marke",
    "Thème à partir de votre marque",
    "Tema a partir de tu marca",
  ],
  brandIntro: [
    "Choose your logo and a few brand images. Veyndo reads their colours and builds a complete theme with readable contrast.",
    "Wähle dein Logo und einige Markenbilder. Veyndo liest ihre Farben und erstellt ein vollständiges Theme mit gut lesbaren Kontrasten.",
    "Choisissez votre logo et quelques images de marque. Veyndo lit leurs couleurs et crée un thème complet aux contrastes lisibles.",
    "Elige tu logotipo y algunas imágenes de marca. Veyndo lee sus colores y crea un tema completo con contraste legible.",
  ],
  sources: [
    "Brand images",
    "Markenbilder",
    "Images de marque",
    "Imágenes de marca",
  ],
  companyLogo: [
    "Company logo",
    "Firmenlogo",
    "Logo de l’entreprise",
    "Logotipo de la empresa",
  ],
  productImage: [
    "Product image",
    "Produktbild",
    "Image produit",
    "Imagen de producto",
  ],
  uploadImages: [
    "Upload images",
    "Bilder hochladen",
    "Téléverser des images",
    "Subir imágenes",
  ],
  uploadHint: [
    "PNG, JPEG or WebP. Images are read in your browser and are not stored.",
    "PNG, JPEG oder WebP. Bilder werden in deinem Browser gelesen und nicht gespeichert.",
    "PNG, JPEG ou WebP. Les images sont lues dans votre navigateur et ne sont pas enregistrées.",
    "PNG, JPEG o WebP. Las imágenes se leen en tu navegador y no se guardan.",
  ],
  uploadedImage: [
    "Uploaded image",
    "Hochgeladenes Bild",
    "Image téléversée",
    "Imagen subida",
  ],
  noSources: [
    "Choose at least one image.",
    "Wähle mindestens ein Bild.",
    "Choisissez au moins une image.",
    "Elige al menos una imagen.",
  ],
  imageError: [
    "An image could not be read. Use PNG, JPEG or WebP.",
    "Ein Bild konnte nicht gelesen werden. Verwende PNG, JPEG oder WebP.",
    "Une image n’a pas pu être lue. Utilisez PNG, JPEG ou WebP.",
    "No se pudo leer una imagen. Usa PNG, JPEG o WebP.",
  ],
  generate: ["Create theme", "Theme erstellen", "Créer le thème", "Crear tema"],
  palette: [
    "Colours found",
    "Gefundene Farben",
    "Couleurs trouvées",
    "Colores encontrados",
  ],
  typography: ["Typography", "Typografie", "Typographie", "Tipografía"],
  classic: ["Classic", "Klassisch", "Classique", "Clásica"],
  editorial: ["Editorial", "Redaktionell", "Éditorial", "Editorial"],
  modern: ["Modern", "Modern", "Moderne", "Moderna"],
  friendly: ["Friendly", "Freundlich", "Chaleureuse", "Cercana"],
  crafted: ["Crafted", "Handwerklich", "Artisanale", "Artesanal"],
  timeless: ["Timeless", "Zeitlos", "Intemporelle", "Atemporal"],
  technical: ["Technical", "Technisch", "Technique", "Técnica"],
  corners: ["Corners", "Ecken", "Coins", "Esquinas"],
  sharp: ["Sharp", "Kantig", "Nets", "Rectas"],
  soft: ["Soft", "Weich", "Doux", "Suaves"],
  round: ["Round", "Rund", "Arrondis", "Redondeadas"],
  spacing: ["Spacing", "Abstände", "Espacement", "Espaciado"],
  compact: ["Compact", "Kompakt", "Compact", "Compacto"],
  regular: ["Regular", "Normal", "Normal", "Normal"],
  airy: ["Airy", "Luftig", "Aéré", "Amplio"],
  motion: ["Motion", "Bewegung", "Animation", "Movimiento"],
  off: ["Off", "Aus", "Désactivée", "Desactivado"],
  subtle: ["Subtle", "Dezent", "Discrète", "Sutil"],
  expressive: ["Expressive", "Ausdrucksstark", "Expressive", "Expresivo"],
  inheritMotion: ["Inherited", "Geerbt", "Héritée", "Heredado"],
  motionHint: [
    "Visitors who prefer reduced motion always see a still shop.",
    "Wer reduzierte Bewegung bevorzugt, sieht den Shop immer ohne Animation.",
    "Les visiteurs qui préfèrent réduire les animations voient toujours une boutique immobile.",
    "Quien prefiere reducir el movimiento siempre ve la tienda sin animaciones.",
  ],
  contrastOk: [
    "All {count} colour pairs meet WCAG AA contrast.",
    "Alle {count} Farbpaare erfüllen den WCAG-AA-Kontrast.",
    "Les {count} paires de couleurs respectent le contraste WCAG AA.",
    "Los {count} pares de colores cumplen el contraste WCAG AA.",
  ],
  contrastFail: [
    "Some colour pairs miss WCAG AA contrast. Create the theme again.",
    "Einige Farbpaare verfehlen den WCAG-AA-Kontrast. Erstelle das Theme erneut.",
    "Certaines paires n’atteignent pas le contraste WCAG AA. Recréez le thème.",
    "Algunos pares no alcanzan el contraste WCAG AA. Crea el tema de nuevo.",
  ],
  currentDesign: [
    "Current design",
    "Aktuelle Gestaltung",
    "Design actuel",
    "Diseño actual",
  ],
  brandDesign: [
    "Brand theme",
    "Marken-Theme",
    "Thème de marque",
    "Tema de marca",
  ],
  applyBrand: [
    "Apply to draft",
    "In den Entwurf übernehmen",
    "Appliquer au brouillon",
    "Aplicar al borrador",
  ],
  applyNote: [
    "This changes only your private draft. Save it and publish it in the release review; the live shop stays unchanged until then.",
    "Das ändert nur deinen privaten Entwurf. Speichere ihn und veröffentliche ihn in der Freigabeprüfung; bis dahin bleibt der Live-Shop unverändert.",
    "Seul votre brouillon privé change. Enregistrez-le et publiez-le dans la revue de publication ; la boutique en ligne reste inchangée d’ici là.",
    "Solo cambia tu borrador privado. Guárdalo y publícalo en la revisión de publicación; la tienda en vivo no cambia hasta entonces.",
  ],
  cancel: ["Cancel", "Abbrechen", "Annuler", "Cancelar"],
  brandActive: [
    "Brand theme active",
    "Marken-Theme aktiv",
    "Thème de marque actif",
    "Tema de marca activo",
  ],
  editBrand: [
    "Edit brand theme",
    "Marken-Theme bearbeiten",
    "Modifier le thème de marque",
    "Editar tema de marca",
  ],
  removeBrand: [
    "Remove brand theme",
    "Marken-Theme entfernen",
    "Retirer le thème de marque",
    "Quitar tema de marca",
  ],
  homeOnly: [
    "Brand themes and motion apply to the whole shop and are set on the home page.",
    "Marken-Themes und Bewegung gelten für den ganzen Shop und werden auf der Startseite festgelegt.",
    "Les thèmes de marque et les animations s’appliquent à toute la boutique et se règlent sur la page d’accueil.",
    "Los temas de marca y el movimiento se aplican a toda la tienda y se configuran en la página de inicio.",
  ],
} as const;
export type BrandWord = keyof typeof words;
export function useBrandText() {
  const { locale } = useLocale();
  const i =
    ({ en: 0, de: 1, fr: 2, es: 3 } as Record<string, number>)[
      locale.slice(0, 2)
    ] ?? 0;
  return (key: BrandWord) => words[key][i];
}
