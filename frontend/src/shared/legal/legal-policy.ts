/** The storefront's current legal policy as React context: the shop's privacy provider supplies it, product and checkout views read it. */
import { createContext, useContext } from "react";
import type { LegalPolicy } from "./legal-types";

export const LegalPolicyContext = createContext<LegalPolicy | undefined>(
  undefined,
);
export const useLegalPolicy = () => useContext(LegalPolicyContext);
