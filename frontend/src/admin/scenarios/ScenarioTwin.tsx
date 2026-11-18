/** Readiness of the selected private copy as a shop twin; every figure is a native read and each open step links to its owner. */
import { useEffect, useState } from "react";
import type { RequestFn } from "../shell/studio-types";
import type { Tab } from "../shell/navigation";
import { useLocale } from "../../shared/i18n/i18n";
import Icon from "../../shared/ui/Icon";
import { useTwinText } from "./scenario-twin-i18n";
import type { TwinReadiness } from "./scenario-types";
import "./scenario-labs.css";
export default function ScenarioTwin({
  request,
  refresh,
  access,
  onOpen,
}: {
  request: RequestFn;
  refresh: number;
  access: string[];
  onOpen: (tab: Tab) => void;
}) {
  const t = useTwinText(),
    { locale } = useLocale();
  const [value, setValue] = useState<TwinReadiness>(),
    [error, setError] = useState("");
  useEffect(() => {
    let active = true;
    setError("");
    request("/api/agent/scenario-readiness")
      .then((next) => {
        // An older server or a foreign response shape shows nothing rather than wrong figures.
        const twin = next as Partial<TwinReadiness> | undefined;
        if (active)
          setValue(
            twin?.catalogue && twin.checkout && twin.tests && twin.sandbox
              ? (twin as TwinReadiness)
              : undefined,
          );
      })
      .catch((e) => {
        if (active) {
          setValue(undefined);
          setError((e as Error).message);
        }
      });
    return () => {
      active = false;
    };
  }, [request, refresh]);
  if (error)
    return (
      <p className="scenario-error" role="alert">
        {error}
      </p>
    );
  if (!value) return <p role="status">{t("twinLoading")}</p>;
  const { catalogue, checkout, tests, blockers } = value;
  let country = checkout.country ?? "";
  try {
    if (country)
      country =
        new Intl.DisplayNames([locale], { type: "region" }).of(country) ??
        country;
  } catch {
    /* Keep the ISO code when the runtime has no display name. */
  }
  const blocked = (code: string) => blockers.includes(code);
  const steps: {
    id: string;
    title: string;
    done: boolean;
    text: string;
    action?: { label: string; tab: Tab; right: string };
  }[] = [
    {
      id: "catalogue",
      title: t("twinCatalogue"),
      done: !blocked("no_products") && !blocked("channel_empty"),
      text: blocked("no_products")
        ? t("twinCatalogueEmpty")
        : blocked("channel_empty")
          ? t("twinChannelEmpty")
          : t("twinCatalogueOk", { count: catalogue.products }),
      action: {
        label: t("twinBring"),
        tab: "channels",
        right: "channels.read",
      },
    },
    {
      id: "stock",
      title: t("twinStock"),
      done: catalogue.withStock > 0,
      text:
        catalogue.withStock > 0
          ? t("twinStockOk", {
              with: catalogue.withStock,
              without: catalogue.withoutStock,
            })
          : t("twinStockEmpty"),
      action: {
        label: t("twinOpenProducts"),
        tab: "productData",
        right: "catalog.read",
      },
    },
    {
      id: "checkout",
      title: t("twinCheckout"),
      done:
        !!checkout.country && checkout.simulatedPayment && checkout.shipping,
      text: !checkout.country
        ? t("twinCheckoutCountry")
        : !checkout.simulatedPayment
          ? t("twinCheckoutPayment", { country })
          : !checkout.shipping
            ? t("twinCheckoutShipping", { country })
            : t("twinCheckoutOk", { country }),
      action: {
        label: t("twinOpenSettings"),
        tab: "commerce",
        right: "settings.read",
      },
    },
    {
      id: "tests",
      title: t("twinTests"),
      done: tests.customerTests > 0,
      text:
        tests.customerTests > 0
          ? t("twinTestsOk", { count: tests.customerTests })
          : t("twinTestsNone"),
    },
  ];
  return (
    <section className="scenario-twin">
      <header>
        <span className="scenario-twin-emblem">
          <Icon name="layers" size={20} />
        </span>
        <div>
          <h3>
            {t("twinTitle")} · {value.sandbox.name}
          </h3>
          <p>{t("twinIntro")}</p>
        </div>
        <span
          className={`scenario-state ${value.ready ? "is-attack-held" : "is-attack-review"}`}
          role="status"
        >
          {t(value.ready ? "twinReady" : "twinNotReady")}
        </span>
      </header>
      <ol className="scenario-twin-steps">
        {steps.map((step) => (
          <li key={step.id} className={step.done ? "is-done" : "is-open"}>
            <span className="scenario-twin-mark" aria-hidden="true">
              <Icon name={step.done ? "check" : "arrow"} size={14} />
            </span>
            <div>
              <strong>{step.title}</strong>
              <span className="scenario-twin-state">
                {t(step.done ? "twinDone" : "twinOpen")}
              </span>
              <p>{step.text}</p>
            </div>
            {!step.done &&
              step.action &&
              access.includes(step.action.right) && (
                <button
                  className="studio-secondary"
                  onClick={() => onOpen(step.action!.tab)}
                >
                  {step.action.label}
                </button>
              )}
          </li>
        ))}
      </ol>
    </section>
  );
}
