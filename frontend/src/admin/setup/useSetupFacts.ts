/** Read only current native setup data; unavailable sources never become successful checks. */
import { useRead } from "../shell/useRead";
import { useEffect, useState } from "react";
import { useStudio } from "../shell/StudioContext";
import type { CompanyData } from "../settings/company-types";
import type { Config } from "../../shared/api/shop-api";
export function useSetupFacts(enabled = true, changed = 0) {
  const { request, access } = useStudio();
  const [revision, setRevision] = useState(0);
  const path = <T extends string>(right: string, value: T) =>
    enabled && access.includes(right) ? value : null;
  const company = useRead(
    request,
    path("settings.read", "/api/settings/master-data"),
  );
  const commerce = useRead(
    request,
    path("settings.read", "/api/merchant/commerce"),
  );
  const products = useRead(
    request,
    path("catalog.read", "/api/merchant/products?limit=1"),
  );
  const reads = [company, commerce, products];
  // A change or an explicit refresh reads the sources again, keeping the last facts.
  useEffect(() => {
    if (changed || revision)
      for (const read of reads) if (read.status !== "idle") void read.refetch();
  }, [changed, revision]);
  // Facts appear once every enabled source answered or failed.
  const enabledReads = [
    path("settings.read", "on"),
    path("settings.read", "on"),
    path("catalog.read", "on"),
  ];
  const settled = reads.every(
    (read, index) =>
      !enabledReads[index] ||
      read.status === "success" ||
      read.status === "error",
  );
  const result = settled
    ? {
        company: company.data?.data as CompanyData | undefined,
        commerce: commerce.data?.data as Config | undefined,
        products: products.data
          ? products.data.elements?.length > 0
          : undefined,
        failed: reads.some((read) => read.status === "error"),
      }
    : undefined;
  return {
    facts: enabled ? result : undefined,
    refresh: () => setRevision((n) => n + 1),
  };
}
