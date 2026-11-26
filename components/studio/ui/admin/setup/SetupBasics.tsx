/** A focused first company form reuses native fields, geography, revisioned settings and the shared save bar. */
import { useRead } from "../../../../../frontend/src/admin/shell/useRead";
import { Notice } from "../../../../../frontend/src/kit/ui";
import { useEffect } from "react";
import { useStudio } from "../../../../../frontend/src/admin/shell/StudioContext";
import { useSettingsDraft } from "../../../../shop/ui/admin/settings/useSettingsDraft";
import CompanyField from "../../../../shop/ui/admin/settings/CompanyField";
import SettingsSaveBar from "../../../../shop/ui/admin/settings/SettingsSaveBar";
import type { CompanyData } from "../../../../shop/ui/admin/settings/company-types";
import type { CountryCatalogue } from "../../../../../frontend/src/shared/geography/geography-types";
import type { CompanyWord } from "../../../../../frontend/src/shared/i18n/company-i18n";
import { useSetupText } from "./setup-i18n";
import "../../../../shop/ui/admin/settings/company-settings.css";
export default function SetupBasics({
  onDirty,
  onSaved,
}: {
  onDirty: (dirty: boolean) => void;
  onSaved: () => void;
}) {
  const { request, access } = useStudio(),
    t = useSetupText();
  const state = useSettingsDraft<CompanyData>(
    request,
    "/api/settings/master-data",
  );
  const worldRead = useRead<CountryCatalogue>(request, "/store-api/countries");
  const valid = Array.isArray(worldRead.data?.countries);
  const world = valid ? worldRead.data : undefined;
  const worldError =
    worldRead.status === "error" || (!!worldRead.data && !valid);
  const canWrite = access.includes("settings.write");
  useEffect(() => {
    onDirty(state.dirty || state.busy);
  }, [state.dirty, state.busy, onDirty]);
  useEffect(() => {
    if (state.saved) onSaved();
  }, [state.saved, onSaved]);
  if (!state.value)
    return (
      <p role={state.error ? "alert" : "status"}>
        {state.error || t("loading")}
      </p>
    );
  const fields: CompanyWord[] = [
    "name",
    "email",
    "street",
    "houseNumber",
    "postalCode",
    "city",
    "country",
  ];
  return (
    <form
      className="setup-basics"
      onSubmit={(e) => {
        e.preventDefault();
        if (canWrite) void state.save();
      }}
    >
      {(state.error || worldError) && (
        <Notice tone="danger">{state.error || t("factsError")}</Notice>
      )}
      <fieldset disabled={!canWrite || state.busy}>
        <div className="setup-basics-fields">
          {fields.map((field) => (
            <CompanyField
              key={field}
              field={field}
              data={state.value!.data}
              base={{}}
              channel={false}
              world={world}
              onChange={state.change}
            />
          ))}
        </div>
      </fieldset>
      <SettingsSaveBar {...state} canWrite={canWrite} />
    </form>
  );
}
