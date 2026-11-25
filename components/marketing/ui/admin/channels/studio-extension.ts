/** Studio extension of sales channels. */
import { lazy } from "react";
import { defineStudioExtension } from "../../../../../frontend/src/admin/shell/registry/define";

export default defineStudioExtension({
  id: "channels",
  routes: [
    {
      id: "channels",
      position: 3,
      icon: "layers",
      group: "experiences",
      rights: { any: ["channels.read"] },
      component: lazy(() => import("./ChannelsRoute")),
    },
  ],
});
