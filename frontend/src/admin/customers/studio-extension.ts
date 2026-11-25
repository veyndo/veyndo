/** Studio extension of customer management. */
import { lazy } from "react";
import { defineStudioExtension } from "../shell/registry/define";

export default defineStudioExtension({
  id: "customers",
  routes: [
    {
      id: "customers",
      icon: "people",
      group: "commerce",
      rights: { all: ["customers.pii"], any: ["customers.read"] },
      component: lazy(() => import("./CustomersRoute")),
    },
  ],
});
