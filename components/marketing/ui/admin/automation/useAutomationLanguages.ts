/** The current shop's content-language context; a failed read reports its message once. */
import { useEffect } from "react";
import type { RequestFn } from "../../../../../frontend/src/admin/shell/studio-types";
import { useRead } from "../../../../../frontend/src/admin/shell/useRead";

export function useAutomationLanguages(
  request: RequestFn,
  onError: (message: string) => void,
) {
  const context = useRead(request, "/store-api/context");
  const failure = (context.error as Error | undefined)?.message;
  useEffect(() => {
    if (failure) onError(failure);
  }, [failure, onError]);
  return context.data
    ? {
        mainLocale: context.data.mainLocale as string,
        locales: context.data.availableLocales as string[],
      }
    : null;
}
