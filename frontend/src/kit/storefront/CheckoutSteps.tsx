/** Checkout steps part: where the buyer is; it never implies that a payment is confirmed. */
import { z } from "zod";
import { definePart, locale } from "./contract";
import { storefrontText } from "./storefront-i18n";

const steps = ["details_step", "review_step", "complete_step"] as const;

export const checkoutSteps = definePart({
  id: "checkout-steps",
  version: 1,
  schema: z.object({
    locale,
    current: z.enum(["details", "review", "complete"]).default("details"),
  }),
  config: z.object({
    current: z.enum(["details", "review", "complete"]).default("details"),
  }),
  Component: function CheckoutSteps({ locale, current }) {
    const t = storefrontText(locale);
    const index = ["details", "review", "complete"].indexOf(current);
    return (
      <ol className="vy-sf-steps" aria-label={t("checkoutSteps")}>
        {steps.map((key, position) => (
          <li
            key={key}
            data-done={position < index || undefined}
            data-active={position <= index || undefined}
            aria-current={position === index ? "step" : undefined}
          >
            <span aria-hidden="true">
              {position < index ? "✓" : position + 1}
            </span>
            {t(key)}
          </li>
        ))}
      </ol>
    );
  },
});
