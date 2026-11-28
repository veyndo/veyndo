/** The app sandbox panel shows declared rights beside recorded activity and rolls back only after explicit confirmation. */
import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { it, expect, vi } from "vitest";
import { QueryProvider } from "../../src/kit/query";
import { LocaleProvider } from "../../src/shared/i18n/i18n";
import AppSandbox, {
  type Sandbox,
} from "../../../components/apps/ui/admin/apps/AppSandbox";

const sandbox: Sandbox = {
  app: "fixture_ledger",
  version: "1.1.0",
  publisher: { publisher: "fixture", keyId: "first", channel: "stable" },
  declared: {
    permissions: ["data.read", "data.write"],
    egress: { service: false, approved: false, origins: [] },
    data: [{ entity: "ledger", fields: 2, public: false }],
    ports: {
      provides: [
        { name: "fixture.ledger", version: "1.1.0", action: "entries" },
      ],
      consumes: [{ name: "fixture.rates", version: "^1", provider: null }],
    },
    tools: [
      {
        name: "save_entry",
        declared: false,
        cost: "standard",
        requiresApproval: true,
        facts: {
          audience: "merchant",
          permissions: ["catalog"],
          writes: true,
          reversibility: { kind: "irreversible" },
          risk: "high",
          data: { entities: ["app"], pii: false, money: false, stock: false },
          effects: [],
          requires: [],
          conditions: [],
        },
      },
    ],
  },
  observed: {
    calls: [
      { id: 7, action: "entries", status: 200, at: "2026-11-27T10:00:00Z" },
    ],
    egress: [],
    writes: [
      {
        entity: "ledger",
        record: "first",
        revision: 2,
        source: "migration",
        at: "2026-11-27T10:01:00Z",
      },
    ],
  },
  versions: [
    {
      version: "1.1.0",
      digest: "b",
      installedAt: "2026-11-27T10:02:00Z",
      current: true,
      rollback: false,
    },
    {
      version: "1.0.0",
      digest: "a",
      installedAt: "2026-11-27T09:00:00Z",
      current: false,
      rollback: true,
    },
  ],
};

it("shows what an app may do and did, and rolls back only after confirmation", async () => {
  const user = userEvent.setup();
  const request = vi.fn(async (path: string, body?: unknown) => {
    if (path.endsWith("/rollback"))
      return {
        installed: true,
        version: (body as { version: string }).version,
      };
    return sandbox;
  });
  render(
    <LocaleProvider>
      <QueryProvider scope="shop-test">
        <AppSandbox app="fixture_ledger" request={request as never} />
      </QueryProvider>
    </LocaleProvider>,
  );
  expect(
    await screen.findByText("Sandbox and permissions"),
  ).toBeInTheDocument();
  expect(screen.getByText(/Signed by fixture/)).toBeInTheDocument();
  expect(screen.getByText(/No network access/)).toBeInTheDocument();
  expect(screen.getByText(/no provider installed/)).toBeInTheDocument();
  expect(screen.getAllByText(/high risk/).length).toBe(2);
  expect(
    screen.getByText(/treated as high risk with approval required/),
  ).toBeInTheDocument();
  expect(screen.getByText("ledger/first")).toBeInTheDocument();
  await user.click(
    screen.getByRole("button", { name: "Roll back to this version" }),
  );
  expect(request).not.toHaveBeenCalledWith(
    "/api/apps/fixture_ledger/rollback",
    expect.anything(),
  );
  expect(
    screen.getByText(/stored version 1.0.0 is installed again/),
  ).toBeInTheDocument();
  const confirm = screen
    .getAllByRole("button", { name: "Roll back to this version" })
    .at(-1)!;
  await user.click(confirm);
  expect(request).toHaveBeenCalledWith("/api/apps/fixture_ledger/rollback", {
    version: "1.0.0",
    approve: true,
  });
  expect(
    await screen.findByText("Version 1.0.0 is installed again."),
  ).toBeInTheDocument();
});
