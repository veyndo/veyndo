/** Browser sessions are admitted only on the configured Studio origin. Another host (the service directory) asks the server where that is, so sign-in is sent there instead of failing as missing access. */
import { useEffect, useState } from "react";
import { browserRequest } from "./browser-session";

/** The server's wire detail for a browser request on a host that does not admit sessions. */
export const originNotAdmitted = "Browser session origin is not admitted";
const header = "x-veyndo-studio-origin";

/** The Studio origin a refused browser request names, when it differs from this page's origin. */
export function namedStudioOrigin(response: Response): string | undefined {
  const value = response.headers.get(header);
  if (response.status !== 403 || !value) return undefined;
  try {
    const url = new URL(value);
    return url.protocol === "https:" && url.origin !== location.origin
      ? url.origin
      : undefined;
  } catch {
    return undefined;
  }
}

/** Probes this host with a passive session read; undefined when it admits browser sessions or cannot tell. */
export async function probeStudioOrigin(): Promise<string | undefined> {
  const path = "/api/auth/session";
  try {
    const response = await fetch(
      path,
      browserRequest(path, { cache: "no-store", method: "GET" }),
    );
    return namedStudioOrigin(response);
  } catch {
    return undefined;
  }
}

/** The same address on the Studio origin; a shop host keeps its shop as `?shop=`. */
export function studioAddress(studio: string, shop?: string): string {
  const search = new URLSearchParams(location.search);
  if (shop && !search.has("shop")) search.set("shop", shop);
  const query = search.toString();
  return `${studio}/${query ? `?${query}` : ""}${location.hash}`;
}

/** The Studio origin to link to, resolved once per mount; undefined while unknown or when this host admits sessions. */
export function useStudioOrigin(enabled = true): string | undefined {
  const [studio, setStudio] = useState<string>();
  useEffect(() => {
    if (!enabled) return;
    let current = true;
    void probeStudioOrigin().then((value) => {
      if (current) setStudio(value);
    });
    return () => {
      current = false;
    };
  }, [enabled]);
  return studio;
}
