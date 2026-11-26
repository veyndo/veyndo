/** Studio extension of the agents view. */
import { lazy } from "react";
import { defineStudioExtension } from "../../../../../frontend/src/admin/shell/registry/define";

export default defineStudioExtension({
  id: "agents",
  routes: [
    {
      id: "agents",
      icon: "agents",
      group: "intelligence",
      requires: "data",
      rights: {
        all: ["catalog.read", "orders.read", "knowledge.read"],
        any: ["assistant.use"],
      },
      component: lazy(() => import("./AgentsRoute")),
    },
  ],
});
