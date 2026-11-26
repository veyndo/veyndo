/** Purpose inventory and provider disclosures edited in the selected content language. */
import {
  Button,
  Field,
  Select,
  TextInput,
} from "../../../../../frontend/src/kit/ui";
import { useLegalText } from "../../../../../frontend/src/shared/i18n/legal-i18n";
import LocalizedField from "../../../../../frontend/src/shared/i18n/LocalizedField";
import {
  purposes,
  type LegalConfig,
} from "../../../../../frontend/src/shared/legal/legal-types";
export default function LegalServices({
  c,
  update,
}: {
  c: LegalConfig;
  update: (patch: Partial<LegalConfig>) => void;
}) {
  const { l } = useLegalText();
  const behavior = c.behaviorAnalytics ?? {
    enabled: true,
    retentionDays: 30,
    eventsPerHour: 500,
  };
  return (
    <>
      <Field label={l("retention")}>
        <TextInput
          type="number"
          min={1}
          max={365}
          value={c.consentDays}
          onChange={(e) => update({ consentDays: Number(e.target.value) })}
        />
      </Field>
      <p>{l("necessaryHint")}</p>
      <article className="legal-service">
        <h3>{l("behaviorSettings")}</h3>
        <p>{l("behaviorHint")}</p>
        <label className="legal-toggle">
          <input
            type="checkbox"
            checked={behavior.enabled}
            onChange={(e) =>
              update({
                behaviorAnalytics: { ...behavior, enabled: e.target.checked },
              })
            }
          />
          {l("behaviorEnabled")}
        </label>
        <Field label={l("behaviorRetention")}>
          <TextInput
            type="number"
            min={1}
            max={30}
            value={behavior.retentionDays}
            onChange={(e) =>
              update({
                behaviorAnalytics: {
                  ...behavior,
                  retentionDays: Number(e.target.value),
                },
              })
            }
          />
        </Field>
        <Field label={l("behaviorAdmission")}>
          <TextInput
            type="number"
            min={1}
            max={500}
            value={behavior.eventsPerHour}
            onChange={(e) =>
              update({
                behaviorAnalytics: {
                  ...behavior,
                  eventsPerHour: Number(e.target.value),
                },
              })
            }
          />
        </Field>
      </article>
      <div className="legal-sector-grid">
        {purposes.map((p) => (
          <label className="legal-toggle" key={p}>
            <input
              type="checkbox"
              checked={!!c.purposes[p]}
              onChange={(e) =>
                update({
                  purposes: {
                    ...c.purposes,
                    [p]: e.target.checked,
                  },
                })
              }
            />
            {l(p)}
          </label>
        ))}
      </div>
      {c.services.map((s, index) => (
        <article className="legal-service" key={s.id}>
          <header>
            <h3>{s.provider || l("service")}</h3>
            <button
              type="button"
              onClick={() =>
                update({
                  services: c.services.filter((v) => v.id !== s.id),
                })
              }
            >
              {l("remove")}
            </button>
          </header>
          {(["provider", "retention", "privacyUrl"] as const).map((k) => (
            <label key={k}>
              {l(k === "retention" ? "serviceRetention" : k)}
              <input
                value={s[k]}
                type={k === "privacyUrl" ? "url" : "text"}
                required
                maxLength={k === "retention" ? 300 : 200}
                onChange={(e) =>
                  update({
                    services: c.services.map((v, n) =>
                      n === index ? { ...v, [k]: e.target.value } : v,
                    ),
                  })
                }
              />
            </label>
          ))}
          <Field label={l("purpose")}>
            <Select
              value={s.purpose}
              onChange={(e) =>
                update({
                  services: c.services.map((v, n) =>
                    n === index
                      ? {
                          ...v,
                          purpose: e.target.value as typeof s.purpose,
                        }
                      : v,
                  ),
                })
              }
            >
              {purposes.map((p) => (
                <option key={p} value={p}>
                  {l(p)}
                </option>
              ))}
            </Select>
          </Field>
          <LocalizedField
            label={l("service")}
            value={s.description}
            multiline
            maxLength={2000}
            onChange={(map) =>
              update({
                services: c.services.map((v, n) =>
                  n === index
                    ? {
                        ...v,
                        description: Object.fromEntries(
                          Object.entries(map).filter(([, v]) => v != null),
                        ) as Record<string, string>,
                      }
                    : v,
                ),
              })
            }
          />
        </article>
      ))}
      <Button
        type="button"
        onClick={() =>
          update({
            services: [
              ...c.services,
              {
                id: `service_${crypto.randomUUID().slice(0, 8)}`,
                provider: "",
                purpose: "analytics",
                description: {},
                retention: "",
                privacyUrl: "",
              },
            ],
          })
        }
      >
        + {l("addService")}
      </Button>
    </>
  );
}
