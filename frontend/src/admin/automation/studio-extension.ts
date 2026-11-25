/** Studio extension of automation flows. */
import { lazy } from "react";
import { defineStudioExtension } from "../shell/registry/define";

export default defineStudioExtension({
  id: "automation",
  routes: [
    {
      id: "automation",
      position: 4,
      icon: "pulse",
      group: "experiences",
      rights: { any: ["automation.read"] },
      component: lazy(() => import("./AutomationRoute")),
    },
  ],
});
