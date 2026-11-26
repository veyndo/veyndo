/** Order operations and native detail navigation; the shared grid owns only list presentation. */
import type { GridCriteria } from "../../../../../frontend/src/shared/grid/types";
import StudioSlot from "../../../../../frontend/src/admin/shell/registry/StudioSlot";
import { useEffect, useState } from "react";
import { useQuery } from "../../../../../frontend/src/kit/query";
import { Button, Notice } from "../../../../../frontend/src/kit/ui";
import { downloadFile } from "../../../../../frontend/src/shared/api/download";
import OrderCreate from "./OrderCreate";
import { useAdminCreationText } from "../../../../../frontend/src/shared/i18n/admin-creation-i18n";
import OrderDirectory from "./OrderDirectory";
import { useOperationsText } from "../../../../../frontend/src/shared/i18n/operations-i18n";
import type { RequestFn } from "../../../../../frontend/src/admin/shell/studio-types";
import type { OpenEntity } from "../../../../../frontend/src/admin/shell/useEntityNavigation";
import OrderDetail from "./OrderDetail";
export default function OrdersManager({
  request,
  headers,
  initialId,
  onEntity,
  onEntityBack,
  onDirtyChange,
}: {
  onDirtyChange?: (dirty: boolean) => void;
  request: RequestFn;
  headers: Record<string, string>;
  initialId?: string;
  onEntity?: OpenEntity;
  onEntityBack?: () => void;
}) {
  const { n } = useAdminCreationText();
  const [creating, setCreating] = useState(false);
  const [gridCursor, setGridCursor] = useState("");
  const [gridCriteria, setGridCriteria] = useState<GridCriteria>({});
  const { o } = useOperationsText();
  const [selected, setSelected] = useState<string>();
  useEffect(() => setSelected(initialId), [initialId]);
  const access = useQuery(["auth", "access"], () =>
    request("/api/auth/access"),
  );
  const rights: string[] = access.data?.permissions ?? [];
  const error = (access.error as Error | undefined)?.message;
  return (
    <div className="studio-page workbench operations">
      <div className="page-intro">
        <h1>{o("orders")}</h1>
        <p>{o("ordersHint")}</p>
        {!selected &&
          !creating &&
          [
            "orders.write",
            "customers.read",
            "customers.pii",
            "catalog.read",
          ].every((right) => rights.includes(right)) && (
            <Button tone="primary" onClick={() => setCreating(true)}>
              {n("newOrder")}
            </Button>
          )}
      </div>
      {error && <Notice tone="danger">{error}</Notice>}
      {creating ? (
        <OrderCreate
          request={request}
          rights={rights}
          onBack={() => setCreating(false)}
          onCreated={(id) => {
            setCreating(false);
            if (onEntity) onEntity("orders", id);
            else setSelected(id);
          }}
        />
      ) : selected ? (
        <>
          <StudioSlot location="admin.order" context={{ orderId: selected }} />
          <StudioSlot
            location="admin.order.general"
            context={{ orderId: selected }}
          />
          <OrderDetail
            onDirtyChange={onDirtyChange}
            id={selected}
            request={request}
            rights={rights}
            onCustomer={(email) => onEntity?.("customers", email)}
            onProduct={(id) => onEntity?.("productData", id)}
            onBack={() =>
              onEntityBack ? onEntityBack() : setSelected(undefined)
            }
            download={(path) => downloadFile(path, headers)}
          />
        </>
      ) : (
        <OrderDirectory
          queryState={{
            criteria: gridCriteria,
            onCriteria: setGridCriteria,
            cursor: gridCursor,
            onCursor: setGridCursor,
          }}
          request={request}
          rights={rights}
          onOpen={(id) => (onEntity ? onEntity("orders", id) : setSelected(id))}
        />
      )}
    </div>
  );
}
