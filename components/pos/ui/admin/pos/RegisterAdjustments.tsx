/** Currency/coupons forward to the shared commerce owners; no local price calculation. */
import { Field, Select, TextInput } from "../../../../../frontend/src/kit/ui";
import { useEffect, useState } from "react";
import type { useRegister } from "./useRegister";
import { useLocale } from "../../../../../frontend/src/shared/i18n/i18n";
import { contentText } from "../../../../../frontend/src/shared/i18n/content-language";
import { usePosText } from "./pos-i18n";
export default function RegisterAdjustments({
  register: r,
  mainLocale,
}: {
  register: ReturnType<typeof useRegister>;
  mainLocale: string;
}) {
  const t = usePosText();
  const { locale } = useLocale();
  const [codes, setCodes] = useState("");
  const c = r.cart;
  const savedCodes = c?.couponCodes?.join(", ") ?? "";
  useEffect(() => setCodes(savedCodes), [c?.id, savedCodes]);
  if (!c) return null;
  return (
    <section className="pos-payment">
      {(c.availableCurrencies?.length ?? 0) > 1 && (
        <Field label={t("currency")}>
          <Select
            value={c.price.currency}
            disabled={r.busy}
            onChange={(e) =>
              void r.adjust("currency", { currency: e.target.value })
            }
          >
            {c.availableCurrencies?.map((code) => (
              <option key={code} value={code}>
                {code}
              </option>
            ))}
          </Select>
        </Field>
      )}
      <details>
        <summary>{t("coupon")}</summary>
        <form
          onSubmit={(e) => {
            e.preventDefault();
            void r.adjust("coupons", {
              codes: codes
                .split(",")
                .map((s) => s.trim())
                .filter(Boolean),
            });
          }}
        >
          <Field label={t("coupon")}>
            <TextInput
              value={codes}
              maxLength={324}
              disabled={r.busy}
              onChange={(e) => setCodes(e.target.value)}
            />
          </Field>
          <button disabled={r.busy}>{t("apply")}</button>
        </form>
      </details>
      {c.discounts?.map((d, i) => (
        <p key={i}>
          {contentText(d.name, locale, mainLocale)} · −
          {new Intl.NumberFormat(locale, {
            style: "currency",
            currency: c.price.currency,
          }).format(d.amount)}
        </p>
      ))}
    </section>
  );
}
