/** Product data route: the catalogue with deep-linked product entities and write access from current rights. */
import ProductDataView from "./ProductDataView";
import { useStudio } from "../../../../../frontend/src/admin/shell/StudioContext";

export default function ProductDataRoute() {
  const { access, request, openEntity, entityTarget, entityBack } = useStudio();
  return (
    <ProductDataView
      canWrite={access.includes("catalog.write")}
      request={request}
      onEntity={openEntity}
      initialId={
        entityTarget?.tab === "productData" ? entityTarget.id : undefined
      }
      onEntityBack={entityTarget ? entityBack : undefined}
    />
  );
}
