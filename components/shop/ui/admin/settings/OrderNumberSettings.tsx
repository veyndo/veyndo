/** Shop-wide order number prefix on the company basis; channels inherit it and cannot override it. */
import { useId } from "react";
import { useCompanyText } from "../../../../../frontend/src/shared/i18n/company-i18n";
import { companyText, type CompanyData } from "./company-types";

/** Capital letters and digits only, at most six, as the server admits them. */
export function normalizeOrderPrefix(text: string) {
  return text
    .toUpperCase()
    .replace(/[^A-Z0-9]/g, "")
    .slice(0, 6);
}

export default function OrderNumberSettings({
  data,
  derived,
  onChange,
}: {
  data: CompanyData;
  derived: string;
  onChange: (d: CompanyData) => void;
}) {
  const { co } = useCompanyText(),
    id = useId(),
    hint = `${id}-hint`;
  const prefix = companyText(data, "orderNumberPrefix");
  return (
    <>
      <legend>{co("orderNumbers")}</legend>
      <p id={hint}>
        {co("orderNumberHint")
          .replace("{prefix}", derived)
          .replace("{example}", `${prefix || derived}-74ab58ea`)}
      </p>
      <div className="customer-field-grid">
        <div className="company-field">
          <div className="company-field-label">
            <label htmlFor={id}>{co("orderNumberPrefix")}</label>
          </div>
          <input
            id={id}
            value={prefix}
            placeholder={derived}
            maxLength={6}
            pattern="[A-Z0-9]{0,6}"
            autoCapitalize="characters"
            spellCheck={false}
            aria-describedby={hint}
            onChange={(e) =>
              onChange({
                ...data,
                orderNumberPrefix: normalizeOrderPrefix(e.target.value),
              })
            }
          />
        </div>
      </div>
    </>
  );
}
