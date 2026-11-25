/** The first-party Studio extensions in route order; every Studio area registers here exactly like an app manifest. */
import setup from "../../../../../components/shop/ui/admin/setup/studio-extension";
import design from "../../../../../components/studio/ui/admin/workspace/studio-extension";
import assistant from "../../../../../components/intelligence/ui/admin/assistant/studio-extension";
import dashboard from "../../../../../components/studio/ui/admin/dashboard/studio-extension";
import intelligence from "../../../../../components/intelligence/ui/admin/intelligence/studio-extension";
import agents from "../../../../../components/intelligence/ui/admin/agents/studio-extension";
import catalog from "../../../../../components/catalog/ui/admin/catalog/studio-extension";
import orders from "../../../../../components/orders/ui/admin/orders/studio-extension";
import customers from "../../../../../components/customers/ui/admin/customers/studio-extension";
import b2b from "../../../../../components/b2b/ui/admin/b2b/studio-extension";
import pos from "../../../../../components/pos/ui/admin/pos/studio-extension";
import channels from "../../../../../components/marketing/ui/admin/channels/studio-extension";
import automation from "../../../../../components/marketing/ui/admin/automation/studio-extension";
import apps from "../../../../../components/apps/ui/admin/apps/studio-extension";
import developer from "../../../../../components/apps/ui/admin/developer/studio-extension";
import environments from "../../../../../components/studio/ui/admin/environments/studio-extension";
import team from "../../../../../components/identity/ui/admin/team/studio-extension";
import settings from "../../../../../components/shop/ui/admin/settings/studio-extension";
import reports from "../../../../../components/studio/ui/admin/reports/studio-extension";
import { createStudioRegistry } from "./registry";
import type { Tab } from "../navigation";

/** Route order decides the first route a person's rights open. */
export const studioRegistry = createStudioRegistry<Tab>([
  setup,
  design,
  assistant,
  dashboard,
  intelligence,
  agents,
  catalog,
  orders,
  customers,
  b2b,
  pos,
  channels,
  automation,
  apps,
  developer,
  environments,
  team,
  settings,
  reports,
]);
