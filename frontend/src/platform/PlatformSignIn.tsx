/** Personal sign-in verifies the current operator grant before retaining a browser session; a host that admits no browser session is reported as such, with the Studio origin to use. */
import Brand from "../shared/ui/Brand";
import { useState } from "react";
import { activateBrowserSession } from "../shared/api/browser-session";
import type { Session } from "../shared/api/account-types";
import { usePlatformText } from "../shared/i18n/platform-i18n";
import {
  PlatformError,
  platformRequest as request,
  type Operator,
} from "./platform-api";
import PlatformLanguage from "./PlatformLanguage";
import RecoveryLink from "../shared/auth/RecoveryLink";
import AuthenticationChallenge, {
  isMfaChallenge,
  type MfaChallenge,
} from "../shared/auth/AuthenticationChallenge";
export default function PlatformSignIn({
  loading,
  error,
  onAuthenticated,
}: {
  loading: boolean;
  error: string;
  onAuthenticated: (token: string) => void;
}) {
  const t = usePlatformText();
  const [challenge, setChallenge] = useState<MfaChallenge>();
  const [email, setEmail] = useState(""),
    [password, setPassword] = useState(""),
    [busy, setBusy] = useState(false),
    [failure, setFailure] = useState(""),
    [studio, setStudio] = useState<string>();
  const login = async () => {
    if (busy || loading) return;
    setBusy(true);
    setFailure("");
    setStudio(undefined);
    let issued = "";
    try {
      const s = await request<Session | MfaChallenge>("", "/api/auth/login", {
        email,
        password,
      });
      if (isMfaChallenge(s)) {
        setChallenge(s);
        setPassword("");
        return;
      }
      if (!s.token) throw new Error(t("retry"));
      activateBrowserSession(s);
      issued = s.token;
      await request<Operator>(issued, "/api/platform/session");
      setPassword("");
      onAuthenticated(issued);
    } catch (e) {
      if (issued) await request(issued, "/api/auth/logout", {}).catch(() => {});
      if (e instanceof PlatformError && e.originNotAdmitted)
        setStudio(e.studioOrigin);
      setFailure(
        e instanceof PlatformError && e.status === 401
          ? t("invalid")
          : e instanceof PlatformError && e.originNotAdmitted
            ? t("originNotAdmitted")
            : e instanceof PlatformError && e.status === 403
              ? t("restricted")
              : t("retry"),
      );
    } finally {
      setBusy(false);
    }
  };
  return (
    <div className="platform-console platform-login">
      <div className="platform-welcome">
        <span className="platform-brand">
          <Brand markOnly />
        </span>
        <h1>{t("title")}</h1>
        <p>{t("subtitle")}</p>
      </div>
      <section className="platform-panel">
        <PlatformLanguage />
        <h2>{t("signin")}</h2>
        {challenge ? (
          <AuthenticationChallenge
            challenge={challenge}
            onCancel={() => setChallenge(undefined)}
            onSession={async (s) => {
              if (!s.token) return;
              activateBrowserSession(s);
              await request<Operator>(s.token, "/api/platform/session");
              onAuthenticated(s.token);
            }}
          />
        ) : (
          <form
            onSubmit={(e) => {
              e.preventDefault();
              void login();
            }}
          >
            <label>
              {t("email")}
              <input
                type="email"
                required
                autoComplete="username"
                value={email}
                onChange={(e) => setEmail(e.target.value)}
              />
            </label>
            <label>
              {t("password")}
              <input
                type="password"
                required
                autoComplete="current-password"
                value={password}
                onChange={(e) => setPassword(e.target.value)}
              />
            </label>
            {(failure || error) && (
              <p role="alert">
                {failure || error}
                {failure && studio && (
                  <>
                    {" "}
                    <a href={`${studio}/#platform`}>
                      {t("signInAt")} {studio}
                    </a>
                  </>
                )}
              </p>
            )}
            <button className="platform-primary" disabled={busy || loading}>
              {busy || loading ? t("loading") : t("login")}
            </button>
            <RecoveryLink />
          </form>
        )}
      </section>
    </div>
  );
}
