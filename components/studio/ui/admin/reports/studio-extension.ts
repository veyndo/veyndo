/** Studio extension of reports: the shop-today report board as a dashboard card. */
import { lazy } from "react";
import {
  contribute,
  defineStudioExtension,
} from "../../../../../frontend/src/admin/shell/registry/define";

export default defineStudioExtension({
  id: "reports",
  contributions: [
    contribute({
      id: "reports.today",
      kind: "dashboard-card",
      location: "admin.dashboard",
      component: lazy(() => import("./TodayReports")),
    }),
  ],
});
