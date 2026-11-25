/** Studio extension of setup: the personal workspace start and the guided shop setup share one entry view. */
import { lazy } from "react";
import { defineStudioExtension } from "../shell/registry/define";

const SetupEntry = lazy(() => import("./SetupEntry"));

export default defineStudioExtension({
  id: "setup",
  routes: [
    {
      id: "workspace",
      icon: "gauge",
      group: "intelligence",
      rights: {
        any: [
          "catalog.read",
          "orders.read",
          "b2b.read",
          "channels.read",
          "settings.read",
          "apps.read",
          "developer.read",
          "team.manage",
          "pos.use",
          "customers.read",
          "knowledge.read",
        ],
      },
      component: SetupEntry,
    },
    {
      id: "setup",
      icon: "check",
      group: "intelligence",
      rights: { all: ["settings.read"], any: ["settings.write"] },
      component: SetupEntry,
    },
  ],
});
