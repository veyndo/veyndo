/** Readable checkout progress (kit checkout-steps part) reflects reviewed server state; it never implies payment confirmation. */
import { useLocale } from "../../shared/i18n/i18n";
import { checkoutSteps } from "../../kit/storefront";
const Steps = checkoutSteps.Component;
export default function CheckoutProgress({
  reviewed,
  complete = false,
}: {
  reviewed: boolean;
  complete?: boolean;
}) {
  const { locale } = useLocale();
  return (
    <Steps
      locale={locale}
      current={complete ? "complete" : reviewed ? "review" : "details"}
    />
  );
}
