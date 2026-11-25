/** Studio extension of payments: the payment ledger panel on the order page. */
import { lazy } from "react";
import {
  contribute,
  defineStudioExtension,
} from "../../../../../frontend/src/admin/shell/registry/define";

export default defineStudioExtension({
  id: "payments",
  contributions: [
    contribute({
      id: "payments.order",
      kind: "page-slot",
      location: "admin.order.payment",
      rights: { any: ["payments.read", "payments.manage"] },
      component: lazy(() => import("./OrderPayment")),
    }),
  ],
});
