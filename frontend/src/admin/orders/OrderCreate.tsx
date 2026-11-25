/** Manual orders remain actor-private drafts until explicit native quote review and idempotent completion. */
import { Button, Card, Field, Notice, Select, TextInput } from "../../kit/ui";
import OrderReview from "./OrderReview";
import useOrderCreation from "./useOrderCreation";
import type { ApiRequestFn as RequestFn } from "../../shared/api/types";
import { addressComplete } from "../../shared/customer/customer-types";
import AddressFields from "../../shared/customer/AddressFields";
import CustomerPicker from "../../shared/customer/CustomerPicker";
import CustomerCreate from "../customers/CustomerCreate";
import ConfirmDialog from "../../shared/ui/ConfirmDialog";
import OrderProductPicker from "./OrderProductPicker";
import "../styles/creation.css";
export default function OrderCreate({
  request,
  rights,
  onBack,
  onCreated,
}: {
  request: RequestFn;
  rights: string[];
  onBack: () => void;
  onCreated: (id: string) => void;
}) {
  const state = useOrderCreation(request);
  const {
    n,
    locale,
    s,
    channels,
    channel,
    setChannel,
    language,
    setLanguage,
    context,
    currency,
    setCurrency,
    customer,
    items,
    setItems,
    selection,
    setSelection,
    billing,
    setBilling,
    shipping,
    setShipping,
    same,
    setSame,
    codes,
    setCodes,
    review,
    setReview,
    busy,
    setBusy,
    error,
    setError,
    discard,
    setDiscard,
    creatingCustomer,
    setCreatingCustomer,
    groups,
    groupName,
    setAuthorization,
    setTerms,
    setDigital,
    setRequestKey,
    pickCustomer,
    addProduct,
    countries,
    dirty,
  } = state;
  if (creatingCustomer)
    return (
      <CustomerCreate
        request={request}
        countries={countries}
        groups={groups}
        groupName={groupName}
        onBack={() => setCreatingCustomer(false)}
        onCreated={async (email) => {
          setCreatingCustomer(false);
          try {
            const c = await request(
              `/api/merchant/customers/${encodeURIComponent(email)}`,
            );
            void pickCustomer(c);
          } catch (e) {
            setError((e as Error).message);
          }
        }}
      />
    );
  return (
    <Card className="admin-create">
      <header className="admin-create-heading">
        <div>
          <h2>{n("newOrder")}</h2>
          <p>{n("orderHint")}</p>
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
      {!review ? (
        <form
          onSubmit={async (e) => {
            e.preventDefault();
            if (
              !customer ||
              !items.length ||
              !addressComplete(billing) ||
              (!same && !addressComplete(shipping))
            ) {
              setError(n("incomplete"));
              return;
            }
            setBusy(true);
            setError("");
            try {
              const address = same ? billing : shipping;
              const next = await request("/api/merchant/order-drafts/review", {
                channelId: channel,
                ...(language ? { locale: language } : {}),
                currency,
                customerId: customer.id,
                items: items.map(({ id, quantity }) => ({ id, quantity })),
                checkout: {
                  ...selection,
                  country: address.country,
                  address,
                  billingAddress: billing,
                },
                codes: codes
                  .split(",")
                  .map((c) => c.trim())
                  .filter(Boolean),
              });
              setReview(next);
              setAuthorization(false);
              setTerms(false);
              setDigital(false);
              setRequestKey(crypto.randomUUID());
            } catch (e) {
              setError((e as Error).message);
            } finally {
              setBusy(false);
            }
          }}
        >
          <fieldset className="admin-create-section" disabled={busy}>
            <legend>{n("configuration")}</legend>
            <div className="customer-field-grid">
              <Field label={n("channel")}>
                <Select
                  value={channel}
                  onChange={(e) => setChannel(e.target.value)}
                >
                  {channels.map((c) => (
                    <option value={c.id} key={c.id}>
                      {c.name}
                    </option>
                  ))}
                </Select>
              </Field>
              <Field label={n("language")}>
                <Select
                  value={language}
                  onChange={(e) => setLanguage(e.target.value)}
                >
                  {(context?.options.locales ?? [locale]).map((l) => (
                    <option value={l} key={l}>
                      {new Intl.DisplayNames(locale, { type: "language" }).of(
                        l,
                      ) ?? l}
                    </option>
                  ))}
                </Select>
              </Field>
              <Field label={n("currency")}>
                <Select
                  value={currency}
                  onChange={(e) => setCurrency(e.target.value)}
                >
                  {context?.currencies.availableCurrencies.map((c) => (
                    <option value={c.code} key={c.code}>
                      {c.code}
                    </option>
                  ))}
                </Select>
              </Field>
            </div>
          </fieldset>
          <section className="admin-create-section">
            <h3>{n("customer")}</h3>
            <CustomerPicker
              request={request}
              value={customer?.id ?? ""}
              onChange={() => {}}
              onSelected={(c) => void pickCustomer(c)}
              disabled={busy}
            />
            {rights.includes("customers.write") && (
              <Button
                type="button"
                disabled={busy}
                onClick={() => setCreatingCustomer(true)}
              >
                {n("newCustomer")}
              </Button>
            )}
          </section>
          <section className="admin-create-section">
            <h3>{n("products")}</h3>
            <OrderProductPicker
              request={request}
              disabled={busy}
              onAdd={addProduct}
            />
            {items.map((i) => (
              <div className="orders-line" key={i.id}>
                <strong>{i.name}</strong>
                <Field label={n("quantity")}>
                  <TextInput
                    type="number"
                    min={1}
                    max={10000}
                    required
                    disabled={busy}
                    value={i.quantity}
                    onChange={(e) =>
                      setItems((old) =>
                        old.map((p) =>
                          p.id === i.id
                            ? { ...p, quantity: Number(e.target.value) }
                            : p,
                        ),
                      )
                    }
                  />
                </Field>
                <Button
                  type="button"
                  disabled={busy}
                  onClick={() =>
                    setItems((old) => old.filter((p) => p.id !== i.id))
                  }
                >
                  {n("remove")}
                </Button>
              </div>
            ))}
          </section>
          <section className="admin-create-section">
            <h3>{n("billing")}</h3>
            <AddressFields
              request={request}
              countries={countries}
              value={billing}
              onChange={setBilling}
              disabled={busy}
              autoCompleteSection="billing"
            />
            <label className="checkbox-label">
              <input
                type="checkbox"
                checked={same}
                disabled={busy}
                onChange={(e) => setSame(e.target.checked)}
              />
              {n("sameAddress")}
            </label>
            {!same && (
              <>
                <h3>{n("shipping")}</h3>
                <AddressFields
                  request={request}
                  countries={countries}
                  value={shipping}
                  onChange={setShipping}
                  disabled={busy}
                  autoCompleteSection="shipping"
                />
              </>
            )}
          </section>
          <fieldset
            className="admin-create-section"
            disabled={busy || !context}
          >
            <legend>{n("methods")}</legend>
            <div className="customer-field-grid">
              <Field label={s("shipping")}>
                <Select
                  value={selection.shippingMethodId}
                  onChange={(e) =>
                    setSelection({
                      ...selection,
                      shippingMethodId: e.target.value,
                    })
                  }
                >
                  {context?.options.shipping
                    .filter((m) =>
                      m.countries.includes(
                        (same ? billing : shipping).country ??
                          selection.country,
                      ),
                    )
                    .map((m) => (
                      <option value={m.id} key={m.id}>
                        {m.name}
                      </option>
                    ))}
                </Select>
              </Field>
              <Field label={s("payment")}>
                <Select
                  value={selection.paymentMethodId}
                  onChange={(e) =>
                    setSelection({
                      ...selection,
                      paymentMethodId: e.target.value,
                    })
                  }
                >
                  {context?.options.payments
                    .filter((p) => ["manual", "simulated"].includes(p.mode))
                    .map((p) => (
                      <option value={p.id} key={p.id}>
                        {p.name}
                      </option>
                    ))}
                </Select>
              </Field>
              <Field label={n("coupons")}>
                <TextInput
                  value={codes}
                  maxLength={1000}
                  onChange={(e) => setCodes(e.target.value)}
                />
              </Field>
            </div>
            <p>{n("paymentHint")}</p>
          </fieldset>
          <footer className="settings-save-bar">
            <span>{n("orderHint")}</span>
            <Button tone="primary" type="submit" disabled={busy || !context}>
              {n(busy ? "loading" : "review")}
            </Button>
          </footer>
        </form>
      ) : (
        <OrderReview state={state} request={request} onCreated={onCreated} />
      )}
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
