/** Knowledge route: the knowledge view over the loaded workspace data with entity links and the product preview. */
import { KnowledgeView } from "./KnowledgeView";
import { useStudio } from "../../../../../frontend/src/admin/shell/StudioContext";

export default function KnowledgeRoute() {
  const {
    openEntity,
    focus,
    data,
    request,
    intent,
    onProduct,
    setPreviewOpen,
  } = useStudio();
  return (
    <KnowledgeView
      onEntity={openEntity}
      focus={focus}
      data={data!}
      request={request}
      onIntent={intent}
      onProduct={(id) => {
        onProduct(id);
        setPreviewOpen(true);
      }}
    />
  );
}
