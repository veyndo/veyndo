/** Storyfronts route: the Storyfront app's Studio view for the current stage. */
import { forApps } from "../../shared/api/types";
import { lazy } from "react";
import { useStudio } from "../shell/StudioContext";

const StoryfrontView = lazy(
  () => import("../../../../extensions/apps/storyfront/studio/StoryfrontView"),
);

export default function StoryfrontsRoute() {
  const { request, environment, workspace, access } = useStudio();
  return (
    <StoryfrontView
      request={forApps(request)}
      workspace={environment || workspace}
      rights={access}
    />
  );
}
