/** Explicit PDP safety/sector facts; no generated warning, certification or origin is invented. */
import {
  productLegalFields,
  useProductLegalText,
} from "../../../../../frontend/src/shared/i18n/product-legal-i18n";
import { useLegalText } from "../../../../../frontend/src/shared/i18n/legal-i18n";
import { useLegalPolicy } from "../../../../../frontend/src/shared/legal/legal-policy";
import { contentText } from "../../../../../frontend/src/shared/i18n/content-language";
import { getContentLocale } from "../../../../../frontend/src/shared/api/shop-api";
export default function ProductSafety({
  value,
}: {
  value?: Record<string, unknown>;
}) {
  const { l } = useLegalText(),
    { p } = useProductLegalText(),
    policy = useLegalPolicy();
  if (!value) return null;
  const fields = productLegalFields
    .map((k) => ({
      k,
      text: contentText(
        (value[k] ?? {}) as Record<string, string>,
        getContentLocale(),
        policy?.mainLocale ?? "en-GB",
      ),
    }))
    .filter((v) => v.text);
  if (!fields.length) return null;
  return (
    <section className="product-compliance">
      <h2>{l("compliance")}</h2>
      <dl>
        {fields.map(({ k, text }) => (
          <div key={k}>
            <dt>{p(k)}</dt>
            <dd>{text}</dd>
          </div>
        ))}
      </dl>
    </section>
  );
}
