/** Orders route: order operations with deep-linked order entities in the current shop or stage. */
import OrdersManager from "./OrdersManager";
import { useStudio } from "../../../../../frontend/src/admin/shell/StudioContext";

export default function OrdersRoute() {
  const {
    entityTarget,
    openEntity,
    entityBack,
    request,
    token,
    environment,
    workspace,
  } = useStudio();
  return (
    <OrdersManager
      initialId={entityTarget?.tab === "orders" ? entityTarget.id : undefined}
      onEntity={openEntity}
      onEntityBack={entityTarget ? entityBack : undefined}
      request={request}
      headers={{
        Authorization: `Bearer ${token}`,
        "x-tenant": environment || workspace,
      }}
    />
  );
}
