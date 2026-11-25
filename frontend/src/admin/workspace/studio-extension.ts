/** Studio extension of the standard storefront design: the site builder's designer inside the current stage. */
import { lazy } from "react";
import { defineStudioExtension } from "../shell/registry/define";

export default defineStudioExtension({
  id: "design",
  routes: [
    {
      id: "design",
      position: 1,
      icon: "palette",
      group: "experiences",
      rights: { all: ["catalog.read"], any: ["channels.read"] },
      component: lazy(() => import("./DesignRoute")),
    },
  ],
});
