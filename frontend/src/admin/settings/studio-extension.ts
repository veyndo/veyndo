/** Studio extension of shop settings. */
import { lazy } from "react";
import { defineStudioExtension } from "../shell/registry/define";

export default defineStudioExtension({
  id: "settings",
  routes: [
    {
      id: "commerce",
      icon: "settings",
      group: "workspace",
      requires: "data",
      rights: { any: ["settings.read"] },
      component: lazy(() => import("./SettingsRoute")),
    },
  ],
});
