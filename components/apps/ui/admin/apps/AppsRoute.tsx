/** Apps route: the app manager for the current stage, with order links. */
import AppsManager from "./AppsManager";
import { useStudio } from "../../../../../frontend/src/admin/shell/StudioContext";

export default function AppsRoute() {
  const { openEntity, environment, workspace, request, token, role, access } =
    useStudio();
  return (
    <AppsManager
      onOrder={(id) => openEntity("orders", id)}
      key={environment || workspace}
      request={request}
      token={token}
      role={role}
      rights={access}
    />
  );
}
