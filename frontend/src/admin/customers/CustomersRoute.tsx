/** Customers route: customer management with deep-linked customer entities. */
import CustomersManager from "./CustomersManager";
import { useStudio } from "../shell/StudioContext";

export default function CustomersRoute() {
  const { request, entityTarget, openEntity, entityBack } = useStudio();
  return (
    <CustomersManager
      request={request}
      initialEmail={
        entityTarget?.tab === "customers" ? entityTarget.id : undefined
      }
      onEntity={openEntity}
      onEntityBack={entityTarget ? entityBack : undefined}
    />
  );
}
