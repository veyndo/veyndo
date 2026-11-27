/** Sign-in on a host that admits no browser session: the directory and router go to the named Studio origin, and the operator sign-in says so instead of "no platform access". */
import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, expect, it, vi } from "vitest";
import { LocaleProvider } from "../../src/shared/i18n/i18n";
import AdminHub from "../../src/platform/AdminHub";
import PlatformSignIn from "../../src/platform/PlatformSignIn";
import ApplicationRouter from "../../src/application/ApplicationRouter";
import {
  namedStudioOrigin,
  originNotAdmitted,
  studioAddress,
} from "../../src/shared/api/studio-origin";
vi.mock("../../src/platform/PlatformConsole", () => ({
  default: () => <section>Platform fixture</section>,
}));
const studio = "https://studio.example.test";
function refused(detail: string, origin?: string) {
  return new Response(JSON.stringify({ errors: [{ detail }] }), {
    status: 403,
    headers: {
      "Content-Type": "application/json",
      ...(origin ? { "x-veyndo-studio-origin": origin } : {}),
    },
  });
}
function transport(response: () => Response) {
  const fetcher = vi.fn(async () => response());
  vi.stubGlobal("fetch", fetcher);
  return fetcher;
}
afterEach(() => {
  vi.unstubAllGlobals();
  history.replaceState({}, "", "/");
});
async function signIn() {
  const user = userEvent.setup();
  await user.type(screen.getByLabelText("Email"), "operator@example.test");
  await user.type(
    screen.getByLabelText("Password"),
    "Synthetic-password-2026!",
  );
  await user.click(screen.getByRole("button", { name: "Sign in" }));
}
it("reports a refused host and links the Studio origin instead of missing platform access", async () => {
  transport(() => refused(originNotAdmitted, studio));
  render(
    <PlatformSignIn loading={false} error="" onAuthenticated={vi.fn()} />,
    {
      wrapper: LocaleProvider,
    },
  );
  await signIn();
  const alert = await screen.findByRole("alert");
  expect(alert).toHaveTextContent(
    "This address does not admit browser sign-in",
  );
  expect(alert).not.toHaveTextContent("no platform access");
  expect(
    screen.getByRole("link", { name: `Sign in at ${studio}` }),
  ).toHaveAttribute("href", `${studio}/#platform`);
});
it("keeps the missing-access message for an admitted host that refuses the account", async () => {
  transport(() => refused("Platform operator access required"));
  render(
    <PlatformSignIn loading={false} error="" onAuthenticated={vi.fn()} />,
    {
      wrapper: LocaleProvider,
    },
  );
  await signIn();
  expect(await screen.findByRole("alert")).toHaveTextContent(
    "This account has no platform access",
  );
  expect(screen.queryByRole("link", { name: /Sign in at/ })).toBeNull();
});
it("links both directory sign-ins to the Studio origin the server names", async () => {
  transport(() => refused(originNotAdmitted, studio));
  render(<AdminHub />, { wrapper: LocaleProvider });
  expect(
    await screen.findByRole("link", { name: /Platform operations/ }),
  ).toHaveAttribute("href", `${studio}/#platform`);
  expect(screen.getByRole("link", { name: /Merchant studio/ })).toHaveAttribute(
    "href",
    `${studio}/#login`,
  );
});
it("leaves an admitted host's console in place and holds a refused one for the redirect", async () => {
  const admitted = transport(
    () =>
      new Response("{}", {
        status: 401,
        headers: { "Content-Type": "application/json" },
      }),
  );
  history.replaceState({}, "", "/#platform");
  const view = render(<ApplicationRouter />, { wrapper: LocaleProvider });
  await screen.findByText("Platform fixture");
  const [path, init] = admitted.mock.calls[0] as unknown as [
    string,
    RequestInit,
  ];
  expect(path).toBe("/api/auth/session");
  expect(new Headers(init.headers).get("x-veyndo-browser-session")).toBe("1");
  view.unmount();
  transport(() => refused(originNotAdmitted, studio));
  render(<ApplicationRouter />, { wrapper: LocaleProvider });
  await screen.findByText("…");
  await new Promise((resolve) => setTimeout(resolve, 20));
  expect(screen.queryByText("Platform fixture")).toBeNull();
});
it("keeps the route and shop when moving to the Studio origin", () => {
  history.replaceState({}, "", "/?studio=1#merchant");
  expect(studioAddress(studio, "nord-atelier")).toBe(
    `${studio}/?studio=1&shop=nord-atelier#merchant`,
  );
  expect(namedStudioOrigin(refused(originNotAdmitted, location.origin))).toBe(
    undefined,
  );
  expect(
    namedStudioOrigin(refused(originNotAdmitted, "http://plain.test")),
  ).toBe(undefined);
});
