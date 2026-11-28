/** The Studio component gallery: the kit catalog's entries, contracts and examples in the light and dark theme, in the Studio language. */
import { Gallery } from "../../../../../../frontend/src/kit/catalog";
import { useLocale } from "../../../../../../frontend/src/shared/i18n/i18n";
import "./ui-catalogue.css";

export default function UICatalogue() {
  const { locale } = useLocale();
  return (
    <div className="studio-page ui-catalogue">
      <Gallery locale={locale} />
    </div>
  );
}
