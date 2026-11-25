/** Studio extension of installed apps and of the Storyfront app's entry. */
import { lazy } from "react";
import { defineStudioExtension } from "../shell/registry/define";

export default defineStudioExtension({
  id: "apps",
  routes: [
    {
      id: "apps",
      position: 5,
      icon: "plus",
      group: "experiences",
      requires: "data",
      rights: { any: ["apps.read"] },
      component: lazy(() => import("./AppsRoute")),
    },
    {
      id: "storyfronts",
      position: 2,
      icon: "layers",
      group: "experiences",
      rights: { any: ["apps.read"] },
      component: lazy(() => import("./StoryfrontsRoute")),
    },
  ],
});
