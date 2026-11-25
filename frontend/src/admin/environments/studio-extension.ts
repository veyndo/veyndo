/** Studio extension of sandbox stages. */
import { lazy } from "react";
import { defineStudioExtension } from "../shell/registry/define";

export default defineStudioExtension({
  id: "environments",
  routes: [
    {
      id: "environments",
      icon: "layers",
      group: "workspace",
      rights: { any: ["environments.read"] },
      component: lazy(() => import("./EnvironmentsRoute")),
    },
  ],
});
