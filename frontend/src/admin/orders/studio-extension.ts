/** Studio extension of order operations. */
import { lazy } from "react";
import { defineStudioExtension } from "../shell/registry/define";

export default defineStudioExtension({
  id: "orders",
  routes: [
    {
      id: "orders",
      icon: "box",
      group: "commerce",
      rights: { all: ["customers.pii"], any: ["orders.read"] },
      component: lazy(() => import("./OrdersRoute")),
    },
  ],
});
