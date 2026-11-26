/** Current native automation grants of the workspace; nothing is granted while they load or fail. */
import type { RequestFn } from "../../../../../frontend/src/admin/shell/studio-types";
import { useRead } from "../../../../../frontend/src/admin/shell/useRead";
export function useAutomationAccess(request: RequestFn) {
  const permissions: string[] =
    useRead(request, "/api/auth/access").data?.permissions ?? [];
  return {
    manage: permissions.includes("automation.manage"),
    canCompare: [
      "automation.read",
      "orders.read",
      "catalog.read",
      "customers.read",
      "customers.pii",
      "payments.read",
      "documents.read",
      "b2b.read",
    ].every((s) => permissions.includes(s)),
  };
}
