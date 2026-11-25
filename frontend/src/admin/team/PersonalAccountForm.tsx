/** PersonalAccountForm: focused account-form view with explicit typed inputs and callbacks. */
import type { ApiPath } from "../../kit/api";
import { Button, Field, TextInput } from "../../kit/ui";
import { useShopText } from "../../shared/i18n/shop-i18n";
import type { Session } from "./UsersManager";

import { shopApi } from "../../shared/api/shop-api";
import "./commerce-manager.css";
import { useEffect, useState } from "react";
import AuthenticationChallenge, {
  isMfaChallenge,
  type MfaChallenge,
} from "../../shared/auth/AuthenticationChallenge";
import RecoveryLink from "../../shared/auth/RecoveryLink";
export type PersonalAccountFormProps = {
  run: (fn: () => Promise<void>) => Promise<void>;
  mode: AccountMode;
  onSession: (s: import("./UsersManager").Session) => void | Promise<void>;
  s: ReturnType<typeof useShopText>["s"];
  busy: boolean;
  email?: string;
};
/** The sign-in forms of a personal Studio account. */
export type AccountMode = "login" | "register" | "join";
const endpoints = {
  login: "/api/auth/login",
  register: "/api/auth/register",
  join: "/api/auth/accept",
} as const satisfies Record<AccountMode, ApiPath>;
export default function PersonalAccountForm({
  run,
  mode,
  onSession,
  s,
  busy,
  email,
}: PersonalAccountFormProps) {
  const [challenge, setChallenge] = useState<MfaChallenge>();
  useEffect(() => setChallenge(undefined), [mode]);
  if (challenge)
    return (
      <AuthenticationChallenge
        challenge={challenge}
        onSession={onSession}
        onCancel={() => setChallenge(undefined)}
      />
    );
  return (
    <form
      className="account-form"
      onSubmit={(e) => {
        e.preventDefault();
        const form = new FormData(e.currentTarget);
        const body = Object.fromEntries(form);
        run(async () => {
          const session = await shopApi<Session | MfaChallenge>(
            endpoints[mode],
            body,
          );
          if (isMfaChallenge(session)) {
            setChallenge(session);
            return;
          }
          await onSession(session);
        });
      }}
    >
      {mode !== "join" && (
        <Field label={s("email")}>
          <TextInput
            defaultValue={email}
            readOnly={!!email}
            type="email"
            name="email"
            autoComplete="email"
            required
            maxLength={254}
          />
        </Field>
      )}
      {mode !== "login" && (
        <Field label={s("accountName")}>
          <TextInput name="name" autoComplete="name" required maxLength={100} />
        </Field>
      )}
      <Field label={s("password")}>
        <TextInput
          type="password"
          name="password"
          autoComplete={mode === "login" ? "current-password" : "new-password"}
          minLength={12}
          maxLength={128}
          required
        />
      </Field>
      {mode === "register" && (
        <>
          <Field label={s("workspaceName")}>
            <TextInput name="workspaceName" required maxLength={100} />
          </Field>
          <Field label={s("workspaceId")}>
            <TextInput
              name="workspaceId"
              required
              minLength={2}
              maxLength={48}
              pattern="[a-z0-9][a-z0-9\-]+"
              placeholder={s("shopSlugExample")}
            />
          </Field>
        </>
      )}
      {mode === "join" && (
        <Field label={s("invitationToken")}>
          <TextInput
            name="invitationToken"
            required
            minLength={64}
            maxLength={64}
            autoComplete="off"
          />
        </Field>
      )}
      <Button tone="primary" type="submit" disabled={busy}>
        {s(mode)}
      </Button>
      {mode === "login" && <RecoveryLink />}
    </form>
  );
}
