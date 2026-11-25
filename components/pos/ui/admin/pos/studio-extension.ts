/** Studio extension of the point of sale. */
import { lazy } from "react";
import { defineStudioExtension } from "../../../../../frontend/src/admin/shell/registry/define";

export default defineStudioExtension({
  id: "pos",
  routes: [
    {
      id: "pos",
      icon: "register",
      group: "commerce",
      rights: { any: ["pos.use"] },
      component: lazy(() => import("./POSRoute")),
    },
  ],
});
