/** Studio extension of the knowledge workspace. */
import { lazy } from "react";
import { defineStudioExtension } from "../../../../../frontend/src/admin/shell/registry/define";

export default defineStudioExtension({
  id: "intelligence",
  routes: [
    {
      id: "knowledge",
      icon: "graph",
      group: "intelligence",
      requires: "data",
      rights: { any: ["knowledge.read"] },
      component: lazy(() => import("./KnowledgeRoute")),
    },
  ],
});
