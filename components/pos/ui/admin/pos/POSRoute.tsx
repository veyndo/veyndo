/** POS route: the register for the current shop or stage, with order links and authenticated receipt downloads. */
import POSWorkspace from "./POSWorkspace";
import { downloadFile } from "../../../../../frontend/src/shared/api/download";
import { useStudio } from "../../../../../frontend/src/admin/shell/StudioContext";

export default function POSRoute() {
  const { environment, workspace, request, selectTab, openEntity, token } =
    useStudio();
  return (
    <POSWorkspace
      key={environment || workspace}
      request={request}
      workspace={environment || workspace}
      onChannels={() => selectTab("channels")}
      onOrder={(id) => openEntity("orders", id)}
      download={(path) =>
        downloadFile(path, {
          Authorization: `Bearer ${token}`,
          "x-tenant": environment || workspace,
        })
      }
    />
  );
}
