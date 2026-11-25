/** Storyfronts route: the Storyfront app's Studio view for the current stage. */
import { lazy } from "react";
import { useStudio } from "../shell/StudioContext";

const StoryfrontView = lazy(
  () => import("../../../../extensions/apps/storyfront/studio/StoryfrontView"),
);

export default function StoryfrontsRoute() {
  const { request, environment, workspace, access } = useStudio();
  return (
    <StoryfrontView
      request={request}
      workspace={environment || workspace}
      rights={access}
    />
  );
}
