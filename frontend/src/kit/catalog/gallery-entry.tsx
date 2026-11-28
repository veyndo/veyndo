/** Build entry of the standalone gallery (`dist/gallery/gallery.js`): renders the gallery in the language named by `?lang=`. */
import { createRoot } from "react-dom/client";
import { Gallery } from "./Gallery";
import { galleryLocales } from "./catalog-i18n";
import { injectCatalogStyles } from "./standalone-styles";

const requested = new URLSearchParams(location.search).get("lang");
const locale =
  galleryLocales.find((candidate) => candidate === requested) ?? "en-GB";
document.documentElement.lang = locale;
injectCatalogStyles();
createRoot(document.getElementById("root")!).render(
  <main>
    <Gallery locale={locale} />
  </main>,
);
