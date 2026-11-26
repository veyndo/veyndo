/** Native CRM creation with explicit address defaults, inline validation and unsaved-change recovery. */
import {
  Button,
  Card,
  Field,
  Notice,
  Select,
  TextInput,
} from "../../../../../frontend/src/kit/ui";
import { useState } from "react";
import CustomerFields from "../../../../../frontend/src/shared/customer/CustomerFields";
import AddressFields from "../../../../../frontend/src/shared/customer/AddressFields";
import {
  emptyAddress,
  type Contact,
} from "../../../../../frontend/src/shared/customer/customer-types";
import { useAdminCreationText } from "../../../../../frontend/src/shared/i18n/admin-creation-i18n";
import { useCustomerText } from "../../../../../frontend/src/shared/i18n/customer-i18n";
import { useOperationsText } from "../../../../../frontend/src/shared/i18n/operations-i18n";
import ConfirmDialog from "../../../../../frontend/src/shared/ui/ConfirmDialog";
import type { ApiRequestFn as RequestFn } from "../../../../../frontend/src/shared/api/types";
import "../../../../../frontend/src/admin/styles/creation.css";
export default function CustomerCreate({
  request,
  countries,
  groups,
  groupName,
  onCreated,
  onBack,
}: {
  request: RequestFn;
  countries: string[];
  groups: { id: string }[];
  groupName: (id: string) => string;
  onCreated: (email: string) => void;
  onBack: () => void;
}) {
  const { n } = useAdminCreationText(),
    { c } = useCustomerText(),
    { o } = useOperationsText();
  const [email, setEmail] = useState(""),
    [profile, setProfile] = useState<Contact>({ name: "" });
  const [group, setGroup] = useState(groups[0]?.id ?? "consumer");
  const [billing, setBilling] = useState(emptyAddress(countries[0])),
    [shipping, setShipping] = useState(emptyAddress(countries[0]));
  const [same, setSame] = useState(true),
    [busy, setBusy] = useState(false),
    [error, setError] = useState(""),
    [discard, setDiscard] = useState(false);
  const [requestKey] = useState(() => crypto.randomUUID());
  const personChanged = (next: Contact) => {
    setProfile(next);
    for (const update of [setBilling, setShipping])
      update((old) =>
        !old.name || old.name === profile.name
          ? {
              ...old,
              name: next.name,
              firstName: next.firstName,
              lastName: next.lastName,
              company:
                !old.company || old.company === profile.company
                  ? next.company
                  : old.company,
            }
          : old,
      );
  };
  const dirty = !!(email || profile.name || billing.street || shipping.street);
  return (
    <Card className="admin-create">
      <header className="admin-create-heading">
        <div>
          <h2>{n("newCustomer")}</h2>
          <p>{n("customerHint")}</p>
        </div>
        <Button
          type="button"
          disabled={busy}
          onClick={() => (dirty ? setDiscard(true) : onBack())}
        >
          {n("cancel")}
        </Button>
      </header>
      {error && <Notice tone="danger">{error}</Notice>}
      <form
        onSubmit={async (e) => {
          e.preventDefault();
          setBusy(true);
          setError("");
          try {
            const next = await request("/api/merchant/customers", {
              email,
              profile,
              customerGroup: group,
              billingAddress: billing,
              shippingAddress: same ? null : shipping,
              requestKey,
            });
            onCreated(next.email);
          } catch (e) {
            setError((e as Error).message);
          } finally {
            setBusy(false);
          }
        }}
      >
        <fieldset disabled={busy} className="admin-create-section">
          <legend>{n("contact")}</legend>
          <div className="customer-field-grid">
            <Field label={c("email")}>
              <TextInput
                type="email"
                required
                autoComplete="email"
                maxLength={254}
                value={email}
                onChange={(e) => setEmail(e.target.value)}
              />
            </Field>
            <Field label={o("group")}>
              <Select value={group} onChange={(e) => setGroup(e.target.value)}>
                {groups.map((g) => (
                  <option key={g.id} value={g.id}>
                    {groupName(g.id)}
                  </option>
                ))}
              </Select>
            </Field>
          </div>
          <CustomerFields
            requiredName
            value={profile}
            onChange={personChanged}
          />
        </fieldset>
        <section className="admin-create-section">
          <h3>{n("billing")}</h3>
          <AddressFields
            request={request}
            value={billing}
            onChange={setBilling}
            countries={countries}
            disabled={busy}
            autoCompleteSection="billing"
          />
          <label className="checkbox-label">
            <input
              type="checkbox"
              disabled={busy}
              checked={same}
              onChange={(e) => setSame(e.target.checked)}
            />
            {n("sameAddress")}
          </label>
        </section>
        {!same && (
          <section className="admin-create-section">
            <h3>{n("shipping")}</h3>
            <AddressFields
              request={request}
              value={shipping}
              onChange={setShipping}
              countries={countries}
              disabled={busy}
              autoCompleteSection="shipping"
            />
          </section>
        )}
        <footer className="settings-save-bar">
          <span>{n("customerHint")}</span>
          <Button tone="primary" type="submit" disabled={busy}>
            {n(busy ? "busy" : "saveCustomer")}
          </Button>
        </footer>
      </form>
      {discard && (
        <ConfirmDialog
          title={n("pending")}
          confirmLabel={n("discard")}
          onCancel={() => setDiscard(false)}
          onConfirm={onBack}
        >
          <p>{n("pending")}</p>
        </ConfirmDialog>
      )}
    </Card>
  );
}
