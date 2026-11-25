/** Studio extension of the product catalogue. */
import { lazy } from "react";
import { defineStudioExtension } from "../shell/registry/define";

export default defineStudioExtension({
  id: "catalog",
  routes: [
    {
      id: "productData",
      icon: "box",
      group: "commerce",
      requires: "connection",
      rights: { any: ["catalog.read"] },
      component: lazy(() => import("./ProductDataRoute")),
    },
  ],
});
