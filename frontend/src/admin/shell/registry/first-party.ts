/** The first-party Studio extensions in route order; every Studio area registers here exactly like an app manifest. */
import setup from "../../setup/studio-extension";
import design from "../../workspace/studio-extension";
import assistant from "../../assistant/studio-extension";
import dashboard from "../../dashboard/studio-extension";
import intelligence from "../../intelligence/studio-extension";
import agents from "../../agents/studio-extension";
import catalog from "../../catalog/studio-extension";
import orders from "../../orders/studio-extension";
import customers from "../../customers/studio-extension";
import b2b from "../../b2b/studio-extension";
import pos from "../../pos/studio-extension";
import channels from "../../channels/studio-extension";
import automation from "../../automation/studio-extension";
import apps from "../../apps/studio-extension";
import developer from "../../developer/studio-extension";
import environments from "../../environments/studio-extension";
import team from "../../team/studio-extension";
import settings from "../../settings/studio-extension";
import reports from "../../reports/studio-extension";
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
