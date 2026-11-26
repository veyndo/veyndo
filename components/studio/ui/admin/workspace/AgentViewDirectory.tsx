/** Composition mounts existing domain directories, filters and native entity editors without a second renderer. */
import { useRead } from "../../../../../frontend/src/admin/shell/useRead";
import { lazy, Suspense } from "react";
import CatalogDirectory from "../../../../catalog/ui/admin/catalog/CatalogDirectory";
import OrderDirectory from "../../../../orders/ui/admin/orders/OrderDirectory";
import CustomerDirectory from "../../../../customers/ui/admin/customers/CustomerDirectory";
import type { Category } from "../../../../catalog/ui/admin/catalog/catalog-model";
import type {
  GridQueryState,
  GridEntity,
} from "../../../../../frontend/src/shared/grid/types";
import type { RequestFn } from "../../../../../frontend/src/admin/shell/studio-types";
import { useCompositionText } from "./composition-i18n";
import type { OpenEntity } from "../../../../../frontend/src/admin/shell/useEntityNavigation";
const ProductDataView = lazy(
  () => import("../../../../catalog/ui/admin/catalog/ProductDataView"),
);
const CustomersManager = lazy(
  () => import("../../../../customers/ui/admin/customers/CustomersManager"),
);
const OrdersManager = lazy(
  () => import("../../../../orders/ui/admin/orders/OrdersManager"),
);

export default function AgentViewDirectory({
  entity,
  request,
  queryState,
  rights,
  selected,
  onSelect,
  headers,
  onDirtyChange,
  onEntity,
}: {
  onEntity: OpenEntity;
  onDirtyChange: (dirty: boolean) => void;
  entity: GridEntity;
  request: RequestFn;
  queryState: GridQueryState;
  rights: string[];
  selected?: string;
  onSelect: (id?: string) => void;
  headers: Record<string, string>;
}) {
  const t = useCompositionText();
  const catalogue = useRead(
    request,
    entity === "products" ? "/api/merchant/categories" : null,
  ).data;
  if (selected)
    return (
      <Suspense fallback={<p role="status">{t("loading")}</p>}>
        {entity === "products" ? (
          <ProductDataView
            onEntity={onEntity}
            onDirtyChange={onDirtyChange}
            key={selected}
            request={request}
            initialId={selected}
            canWrite={rights.includes("catalog.write")}
            onEntityBack={() => onSelect(undefined)}
          />
        ) : entity === "orders" ? (
          <OrdersManager
            onEntity={onEntity}
            onDirtyChange={onDirtyChange}
            key={selected}
            request={request}
            headers={headers}
            initialId={selected}
            onEntityBack={() => onSelect(undefined)}
          />
        ) : (
          <CustomersManager
            onEntity={onEntity}
            onDirtyChange={onDirtyChange}
            key={selected}
            request={request}
            initialEmail={selected}
            onEntityBack={() => onSelect(undefined)}
          />
        )}
      </Suspense>
    );
  if (entity === "orders")
    return (
      <OrderDirectory
        request={request}
        queryState={queryState}
        rights={rights}
        onOpen={onSelect}
      />
    );
  if (entity === "customers")
    return (
      <CustomerDirectory
        request={request}
        queryState={queryState}
        groups={[]}
        groupName={(id) => id}
        onOpen={onSelect}
      />
    );
  return (
    <CatalogDirectory
      request={request}
      queryState={queryState}
      onOpen={onSelect}
      categories={(catalogue?.elements as Category[] | undefined) ?? []}
      mainLocale={catalogue?.mainLocale ?? "en-GB"}
      canWrite={false}
      onNew={() => {}}
    />
  );
}
