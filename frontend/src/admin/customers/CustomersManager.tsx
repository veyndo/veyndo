/** CRM list and editable customer profile with linked order history. */
import type { GridCriteria } from "../../shared/grid/types";
import CoreExtensions from "../../shared/apps/CoreExtensions";
import StudioSlot from "../shell/registry/StudioSlot";
import { useQuery } from "../../kit/query";
import {
  Badge,
  Button,
  Card,
  Checkbox,
  Field,
  Notice,
  Select,
} from "../../kit/ui";
import CustomerCreate from "./CustomerCreate";
import { useAdminCreationText } from "../../shared/i18n/admin-creation-i18n";
import CustomerDirectory from "./CustomerDirectory";
import GuestContact from "./GuestContact";
import AddressBook from "../../shared/customer/AddressBook";
import CustomerFields from "../../shared/customer/CustomerFields";
import { useCustomerText } from "../../shared/i18n/customer-i18n";
import "../../shared/styles/customers.css";

import { useEffect, useState } from "react";
import EntityHistory from "../../shared/history/EntityHistory";
import { useCrmText } from "../../shared/i18n/crm-i18n";
import ConfirmDialog from "../../shared/ui/ConfirmDialog";
import { inheritedText } from "../../shared/geography/geography-types";
import type { OpenEntity } from "../shell/useEntityNavigation";
import { useOperationsText } from "../../shared/i18n/operations-i18n";
import type { RequestFn } from "../shell/studio-types";
import { useUnsavedWorkspace } from "../../shared/ui/useUnsavedWorkspace";
export default function CustomersManager({
  request,
  initialEmail,
  onEntity,
  onEntityBack,
  onDirtyChange,
}: {
  onDirtyChange?: (dirty: boolean) => void;
  request: RequestFn;
  initialEmail?: string;
  onEntity?: OpenEntity;
  onEntityBack?: () => void;
}) {
  const { n } = useAdminCreationText();
  const [creating, setCreating] = useState(false);
  const [gridCursor, setGridCursor] = useState("");
  const [gridCriteria, setGridCriteria] = useState<GridCriteria>({});
  const { o } = useOperationsText();
  const { c } = useCustomerText();
  const { r, locale } = useCrmText();
  const [baseline, setBaseline] = useState(""),
    [pending, setPending] = useState<(() => void) | null>(null);
  const [customer, setCustomer] = useState<any>(),
    [detailError, setError] = useState(""),
    [busy, setBusy] = useState(false);
  // Rights decide which context to read: the creation context for writers, the
  // group list for settings readers; equal keys share Studio's query cache.
  const access = useQuery(["auth", "access"], () =>
    request("/api/auth/access"),
  );
  const permissions: string[] = access.data?.permissions ?? [];
  const canEdit = permissions.includes("customers.write");
  const options = useQuery(["checkout", "options"], () =>
    request("/store-api/checkout/options"),
  );
  const creation = useQuery(
    ["customers", "create-context"],
    () => request("/api/merchant/customers/create-context"),
    { enabled: canEdit },
  );
  const groupList = useQuery(
    ["customers", "groups"],
    () => request("/api/merchant/customer-groups"),
    { enabled: !canEdit && permissions.includes("settings.read") },
  );
  const context = canEdit ? creation.data : groupList.data;
  const groups: any[] = context?.elements ?? [];
  const mainLocale: string = context?.mainLocale ?? "en-GB";
  const countries: string[] =
    canEdit && creation.data
      ? (creation.data.countries ?? ["DE"])
      : (options.data?.countries ?? ["DE"]);
  const error =
    detailError ||
    [access, options, creation, groupList]
      .map((query) => (query.error as Error | undefined)?.message)
      .find(Boolean) ||
    "";
  const editable = canEdit && customer?.guest !== true;
  const [addressDirty, setAddressDirty] = useState(false);
  const dirty =
    !!customer &&
    JSON.stringify([
      customer.profile,
      customer.company,
      customer.customerGroup,
      customer.active,
    ]) !== baseline;
  const navigate = (action: () => void) => {
    if (dirty || addressDirty) setPending(() => action);
    else action();
  };
  useUnsavedWorkspace(dirty || addressDirty, onDirtyChange);
  const detail = async (email: string) => {
    try {
      const next = await request(
        `/api/merchant/customers/${encodeURIComponent(email)}`,
      );
      setCustomer(next);
      setBaseline(
        JSON.stringify([
          next.profile,
          next.company,
          next.customerGroup,
          next.active,
        ]),
      );
      setError("");
    } catch (e) {
      setError((e as Error).message);
    }
  };
  useEffect(() => {
    if (initialEmail) void detail(initialEmail);
    else setCustomer(undefined);
  }, [initialEmail]);
  const groupName = (id: string) =>
    inheritedText(
      groups.find((g) => g.id === id)?.translations ?? {},
      locale,
      mainLocale,
      "name",
    ) || id;
  return (
    <div className="studio-page workbench operations">
      <div className="page-intro">
        <h1>{o("customers")}</h1>
        <p>{o("customersHint")}</p>
        {!customer && !creating && canEdit && (
          <Button tone="primary" onClick={() => setCreating(true)}>
            {n("newCustomer")}
          </Button>
        )}
      </div>
      {error && <Notice tone="danger">{error}</Notice>}
      {creating ? (
        <CustomerCreate
          key={mainLocale}
          request={request}
          countries={countries}
          groups={groups}
          groupName={groupName}
          onBack={() => setCreating(false)}
          onCreated={(email) => {
            setCreating(false);
            if (onEntity) onEntity("customers", email);
            else void detail(email);
          }}
        />
      ) : customer ? (
        <>
          <Button
            onClick={() => {
              navigate(() => {
                if (onEntityBack) onEntityBack();
                else {
                  setCustomer(undefined);
                }
              });
            }}
          >
            {onEntityBack ? r("back") : o("back")}
          </Button>
          <Card level={2}>
            <div className="customer-heading">
              <div>
                <small>
                  {customer.guest
                    ? r("guest")
                    : `${c("customerNumber")} · ${customer.customerNumber}`}
                </small>
                <h2>{customer.profile.name || customer.email}</h2>
                <p>{customer.email}</p>
              </div>
              <Badge
                tone={
                  customer.guest || customer.active ? "positive" : "neutral"
                }
              >
                {customer.guest
                  ? r("guest")
                  : o(customer.active ? "active" : "inactive")}
              </Badge>
            </div>
            {!customer.guest && (
              <form
                onSubmit={async (e) => {
                  e.preventDefault();
                  setBusy(true);
                  try {
                    await request(
                      `/api/merchant/customers/${encodeURIComponent(customer.email)}`,
                      {
                        revision: customer.revision,
                        profile: customer.profile,
                        company: customer.company || null,
                        customerGroup: customer.customerGroup,
                        active: customer.active,
                      },
                      "PUT",
                    );
                    await detail(customer.email);
                  } catch (e) {
                    setError((e as Error).message);
                  } finally {
                    setBusy(false);
                  }
                }}
              >
                <CustomerFields
                  value={{
                    ...customer.profile,
                    company: customer.company ?? "",
                  }}
                  disabled={!editable}
                  onChange={(p) =>
                    setCustomer({ ...customer, profile: p, company: p.company })
                  }
                />
                <Field label={o("group")}>
                  <Select
                    disabled={!editable}
                    value={customer.customerGroup}
                    onChange={(e) =>
                      setCustomer({
                        ...customer,
                        customerGroup: e.target.value,
                      })
                    }
                  >
                    {groups.map((g) => (
                      <option key={g.id} value={g.id}>
                        {groupName(g.id)}
                      </option>
                    ))}
                  </Select>
                </Field>
                <Checkbox
                  label={o("active")}
                  disabled={!editable}
                  checked={customer.active}
                  onChange={(e) =>
                    setCustomer({ ...customer, active: e.target.checked })
                  }
                />
                {editable && (
                  <Button type="submit" tone="primary" disabled={busy}>
                    {o("save")}
                  </Button>
                )}
              </form>
            )}
            {dirty && <p className="muted">{r("addressPending")}</p>}
            {customer.guest ? (
              <GuestContact customer={customer} />
            ) : (
              <AddressBook
                onDirtyChange={setAddressDirty}
                request={request}
                path={`/api/merchant/customers/${encodeURIComponent(customer.email)}/addresses`}
                countries={countries}
                canEdit={editable && !dirty && !busy}
                onChange={() => void detail(customer.email)}
              />
            )}
            {customer.orders && (
              <>
                <h3>{o("orders")}</h3>
                {customer.orders.map((r: any) => (
                  <button
                    type="button"
                    className="operation-row"
                    key={r.id}
                    onClick={() => navigate(() => onEntity?.("orders", r.id))}
                  >
                    <strong>{r.orderNumber}</strong>
                    <span>{o(r.state)}</span>
                    <span>
                      {new Intl.NumberFormat(locale, {
                        style: "currency",
                        currency: r.currency ?? "EUR",
                      }).format(r.cart.price.totalPrice)}
                    </span>
                  </button>
                ))}
              </>
            )}
            {!customer.guest && (
              <CoreExtensions
                objectId={customer.id}
                extensions={customer.extensions}
                omittedExtensions={customer.omittedExtensions}
                request={request}
              />
            )}
            {!customer.guest && (
              <StudioSlot
                location="admin.customer"
                context={{ customerId: customer.id }}
              />
            )}
            {!customer.guest && (
              <EntityHistory
                request={request}
                entity="customer"
                id={customer.email}
                revision={customer.revision}
                dirty={dirty || busy}
                onRestored={() => detail(customer.email)}
              />
            )}
          </Card>
        </>
      ) : (
        <CustomerDirectory
          queryState={{
            criteria: gridCriteria,
            onCriteria: setGridCriteria,
            cursor: gridCursor,
            onCursor: setGridCursor,
          }}
          request={request}
          groups={groups}
          groupName={groupName}
          onOpen={(email) =>
            onEntity ? onEntity("customers", email) : void detail(email)
          }
        />
      )}
      {pending && (
        <ConfirmDialog
          title={r("pending")}
          confirmLabel={r("discard")}
          onCancel={() => setPending(null)}
          onConfirm={() => {
            onDirtyChange?.(false);
            pending();
            setPending(null);
          }}
        >
          <p>{r("dirty")}</p>
        </ConfirmDialog>
      )}
    </div>
  );
}
