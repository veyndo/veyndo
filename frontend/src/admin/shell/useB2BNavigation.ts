/** Display business navigation only after the active workspace confirms its feature switches. */
import { useEffect } from "react";
import { useRead } from "./useRead";
import type { RequestFn } from "../../shared/api/types";

type Features = { enabled: boolean; quotes: boolean; edi: boolean };
export const b2bChanged = "commerce.b2b.changed";

export function useB2BNavigation(request: RequestFn, admitted: boolean) {
  const configuration = useRead(
    request,
    admitted ? "/api/b2b/configuration" : null,
    { body: {} },
  );
  // B2B and app changes elsewhere in Studio ask for the switches again.
  useEffect(() => {
    if (!admitted) return;
    const refresh = () => void configuration.refetch();
    window.addEventListener(b2bChanged, refresh);
    window.addEventListener("commerce.apps.changed", refresh);
    return () => {
      window.removeEventListener(b2bChanged, refresh);
      window.removeEventListener("commerce.apps.changed", refresh);
    };
  }, [admitted, configuration.refetch]);
  const features: Features | undefined = configuration.data?.features;
  return admitted && configuration.status === "success" ? features : undefined;
}
