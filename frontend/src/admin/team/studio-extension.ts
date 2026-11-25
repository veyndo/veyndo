/** Studio extension of team and access management. */
import { lazy } from "react";
import { defineStudioExtension } from "../shell/registry/define";

export default defineStudioExtension({
  id: "team",
  routes: [
    {
      id: "users",
      icon: "lock",
      group: "workspace",
      rights: { any: ["team.manage"] },
      component: lazy(() => import("./TeamRoute")),
    },
  ],
});
