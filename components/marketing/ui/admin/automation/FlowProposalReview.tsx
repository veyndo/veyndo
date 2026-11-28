/** Flow review observes the original native task, including approval made in another Studio view. */
import { Notice } from "../../../../../frontend/src/kit/ui";
import ProposalCard from "../../../../../frontend/src/admin/shell/ProposalCard";
import AgentRunCard from "../../../../../frontend/src/admin/shell/AgentRunCard";
import { useTaskExecution } from "../../../../../frontend/src/admin/shell/useTaskExecution";
import type {
  Preview,
  RequestFn,
} from "../../../../../frontend/src/admin/shell/studio-types";
import { useAgentWorkspaceText } from "../../../../../frontend/src/shared/i18n/agent-workspace-i18n";
export default function FlowProposalReview({
  taskId,
  preview,
  applied,
  executionStarted,
  request,
  manage,
  onTranslations,
}: {
  taskId: string;
  preview: Preview;
  applied: boolean;
  executionStarted: boolean;
  request: RequestFn;
  manage: boolean;
  onTranslations?: (jobId: string) => void;
}) {
  const progress = useTaskExecution(taskId, applied, request, executionStarted);
  const t = useAgentWorkspaceText();
  return (
    <section>
      {progress.run && (
        <AgentRunCard
          value={progress.run}
          canControl={manage}
          control={async (action) => {
            if (manage) await progress.control(action);
          }}
          onTranslations={onTranslations}
        />
      )}
      {progress.error && (
        <Notice tone="danger">
          {progress.error}
          {progress.accepted && (
            <button onClick={progress.reload}>{t("reload")}</button>
          )}
        </Notice>
      )}
      <ProposalCard
        request={request}
        message={{
          id: 0,
          role: "assistant",
          content: preview.proposal.summary,
          applied: progress.applied,
          data: {
            taskId,
            preview: { ...preview, catalogBefore: preview.catalogBefore ?? [] },
          },
        }}
        canApply={manage && !progress.applied && !progress.accepted}
        busy={progress.busy}
        approvalStarted={progress.accepted}
        onApply={() => {
          if (manage) void progress.approve();
        }}
      />
    </section>
  );
}
