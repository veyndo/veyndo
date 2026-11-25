/** Studio extension of the assistant conversation. */
import { lazy } from "react";
import { defineStudioExtension } from "../shell/registry/define";

export default defineStudioExtension({
  id: "assistant",
  routes: [
    {
      id: "assistant",
      icon: "chat",
      group: "intelligence",
      rights: {
        all: ["catalog.read", "orders.read", "knowledge.read"],
        any: ["assistant.use"],
      },
      component: lazy(() => import("../shell/StudioConversation")),
    },
  ],
});
