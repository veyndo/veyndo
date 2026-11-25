/** Studio extension of B2B companies and quotes; the sidebar shows them in their own section when B2B is enabled. */
import { lazy } from "react";
import { defineStudioExtension } from "../shell/registry/define";

const B2BRoute = lazy(() => import("./B2BRoute"));

export default defineStudioExtension({
  id: "b2b",
  routes: [
    {
      id: "b2b",
      icon: "people",
      rights: { any: ["b2b.read"] },
      component: B2BRoute,
    },
    {
      id: "b2bQuotes",
      icon: "chat",
      rights: { all: ["b2b.manage", "customers.pii"], any: ["b2b.read"] },
      component: B2BRoute,
    },
  ],
});
