/** Studio extension of the developer workspace (App Studio, API console, component catalogue). */
import { lazy } from "react";
import { defineStudioExtension } from "../../../../../frontend/src/admin/shell/registry/define";

export default defineStudioExtension({
  id: "developer",
  routes: [
    {
      id: "developers",
      icon: "code",
      group: "workspace",
      rights: { any: ["developer.read", "team.manage"] },
      component: lazy(() => import("./DeveloperRoute")),
    },
  ],
});
