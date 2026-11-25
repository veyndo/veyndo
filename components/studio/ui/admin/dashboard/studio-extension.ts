/** Studio extension of the overview dashboard; its cards come from the dashboard slot. */
import { lazy } from "react";
import { defineStudioExtension } from "../../../../../frontend/src/admin/shell/registry/define";

export default defineStudioExtension({
  id: "dashboard",
  routes: [
    {
      id: "overview",
      icon: "pulse",
      group: "intelligence",
      requires: "data",
      rights: { any: ["overview.read", "orders.read"] },
      component: lazy(() => import("./OverviewRoute")),
    },
  ],
});
