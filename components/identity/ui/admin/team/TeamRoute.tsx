/** Team route: members and shop switching for the signed-in person, and access management once connected. */
import UsersManager, { type Session } from "./UsersManager";
import AccessManager from "./AccessManager";
import { useStudio } from "../../../../../frontend/src/admin/shell/StudioContext";

export default function TeamRoute() {
  const {
    token,
    workspace,
    auth,
    setWorkspace,
    setWorkspaceName,
    setMessages,
    setId,
    setData,
    connected,
    liveRequest,
  } = useStudio();
  return (
    <>
      <UsersManager
        token={token}
        workspace={workspace}
        onSession={(session: Session) => {
          if (session.token) {
            void auth.accept(session);
            return;
          }
          sessionStorage.setItem("rac-user-workspace", session.workspace);
          setWorkspace(session.workspace);
          setWorkspaceName(
            session.workspaces.find((w) => w.id === session.workspace)?.name ??
              session.workspace,
          );
          history.replaceState(null, "", `?shop=${session.workspace}#merchant`);
        }}
        onWorkspace={(id, name) => {
          sessionStorage.setItem("rac-user-workspace", id);
          setWorkspace(id);
          setWorkspaceName(name);
          history.replaceState(null, "", `?shop=${id}#merchant`);
          setMessages([]);
          setId(undefined);
          setData(undefined);
        }}
        onLogout={auth.logout}
      />
      {connected && <AccessManager request={liveRequest} />}
    </>
  );
}
