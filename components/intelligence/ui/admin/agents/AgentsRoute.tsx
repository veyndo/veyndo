/** Agents route: the agents view over the loaded workspace data. */
import { AgentsView } from "./AgentsView";
import { useStudio } from "../../../../../frontend/src/admin/shell/StudioContext";

export default function AgentsRoute() {
  const { data, onExit } = useStudio();
  return <AgentsView data={data!} onStorefront={onExit} />;
}
