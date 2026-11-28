/** Review diagnostics count actual package/page declarations and route work into the existing editors and preview. */
import { render, screen, fireEvent, waitFor } from "@testing-library/react";
import { expect, it, vi } from "vitest";
import { LocaleScope } from "../../src/shared/i18n/i18n";
import AppDiagnostics from "../../../components/apps/ui/admin/developer/AppDiagnostics";
import { template } from "../../../components/apps/ui/admin/developer/app-model";
import DesignReview from "../../../extensions/apps/site-builder/studio/DesignReview";
import { newPage, newBlock } from "../../../extensions/apps/site-builder/model";
import ProposalCard from "../../src/admin/shell/ProposalCard";
it("shows every proposed work step for one explicit approval and never executes while rendering", () => {
  const approve = vi.fn();
  const message: any = {
    id: 1,
    role: "assistant",
    content: "Prepare the private catalogue",
    applied: false,
    data: {
      taskId: "task",
      preview: {
        catalogBefore: [],
        proposal: {
          summary: "Prepare",
          changes: [],
          operations: [
            {
              name: "merchant.product.create",
              label: "Create the coat",
              arguments_json: '{"product":{"id":"coat"}}',
            },
            {
              name: "automation.save",
              label: "Prepare the review flow",
              arguments_json: '{"id":"review"}',
            },
          ],
        },
      },
    },
  };
  render(
    <LocaleScope locale="en-GB">
      <ProposalCard message={message} onApply={approve} busy={false} canApply />
    </LocaleScope>,
  );
  expect(screen.getByText(/Create the coat/)).toBeVisible();
  expect(screen.getByText(/Prepare the review flow/)).toBeVisible();
  expect(approve).not.toHaveBeenCalled();
  fireEvent.click(screen.getByRole("button", { name: /approve/i }));
  expect(approve).toHaveBeenCalledTimes(1);
});
it("shows the typed facts of each proposed step and flags steps without declared facts", () => {
  const facts = (
    name: string,
    risk: "low" | "medium" | "high",
    kind: string,
    extra: Partial<{ pii: boolean; money: boolean; effects: string[] }> = {},
  ) => ({
    name,
    readOnly: false,
    cost: "standard",
    requiresApproval: risk === "high" || kind === "irreversible",
    facts: {
      permissions: ["orders.write"],
      writes: true,
      reversibility:
        kind === "compensatable"
          ? { kind, by: "merchant.history.restore" }
          : { kind },
      risk,
      data: {
        entities: ["order"],
        pii: !!extra.pii,
        money: !!extra.money,
        stock: false,
      },
      effects: extra.effects ?? [],
      conditions: [],
    },
  });
  const message: any = {
    id: 2,
    role: "assistant",
    content: "Settle the order",
    applied: false,
    data: {
      preview: {
        catalogBefore: [],
        proposal: {
          summary: "Settle",
          changes: [],
          operations: [
            {
              name: "merchant.order.transition",
              label: "Mark paid",
              arguments_json: "{}",
            },
            {
              name: "merchant.product.save",
              label: "Fix the title",
              arguments_json: "{}",
            },
            {
              name: "app.erp.sync",
              label: "Sync the ERP",
              arguments_json: "{}",
            },
          ],
        },
      },
    },
  };
  render(
    <LocaleScope locale="de-DE">
      <ProposalCard
        message={message}
        onApply={vi.fn()}
        busy={false}
        canApply
        operationFacts={{
          "merchant.order.transition": facts(
            "merchant.order.transition",
            "high",
            "irreversible",
            {
              pii: true,
              money: true,
              effects: ["email"],
            },
          ),
          "merchant.product.save": facts(
            "merchant.product.save",
            "medium",
            "compensatable",
          ),
        }}
      />
    </LocaleScope>,
  );
  const [transition, save] = screen.getAllByRole("group", {
    name: "Was dieser Schritt bewirkt",
  });
  expect(transition).toHaveTextContent("Braucht deine Freigabe");
  expect(transition).toHaveTextContent("Hohes Risiko");
  expect(transition).toHaveTextContent("Nicht rückgängig machbar");
  expect(transition).toHaveTextContent("Personenbezogene Daten");
  expect(transition).toHaveTextContent("Sendet E-Mails");
  expect(transition).toHaveTextContent("Benötigt: orders.write");
  expect(save).not.toHaveTextContent("Braucht deine Freigabe");
  expect(save).toHaveTextContent("Rückgängig mit merchant.history.restore");
  expect(screen.getByRole("note")).toHaveTextContent(
    "Keine deklarierten Angaben",
  );
});
it("loads operation facts once through the planner read and shows nothing when it fails", async () => {
  const listed = {
    name: "merchant.payment",
    readOnly: false,
    cost: "provider",
    requiresApproval: true,
    facts: {
      permissions: ["payments.manage"],
      writes: true,
      reversibility: { kind: "irreversible" },
      risk: "high",
      data: { entities: ["payment"], pii: false, money: true, stock: false },
      effects: ["payment"],
      conditions: [],
    },
  };
  const request = vi.fn(async (_path: string, body: any) => ({
    result: {
      isError: false,
      structuredContent:
        body.params.name === "merchant.operations.list"
          ? { operations: [listed] }
          : null,
    },
  }));
  const message: any = {
    id: 3,
    role: "assistant",
    content: "Refund",
    applied: false,
    data: {
      preview: {
        catalogBefore: [],
        proposal: {
          summary: "Refund",
          changes: [],
          operations: [
            {
              name: "merchant.payment",
              label: "Refund the order",
              arguments_json: "{}",
            },
          ],
        },
      },
    },
  };
  const card = (fn: any) => (
    <LocaleScope locale="en-GB">
      <ProposalCard
        message={message}
        onApply={vi.fn()}
        busy={false}
        canApply
        request={fn}
      />
    </LocaleScope>
  );
  const { unmount } = render(card(request));
  expect(await screen.findByText("Moves money")).toBeVisible();
  expect(screen.getByText("Needs your approval")).toBeVisible();
  expect(request).toHaveBeenCalledWith(
    "/mcp",
    expect.objectContaining({
      method: "tools/call",
      params: {
        name: "merchant.operations.list",
        arguments: { includeUnavailable: true },
      },
    }),
    "POST",
  );
  unmount();
  const again = render(card(request));
  expect(await screen.findByText("Moves money")).toBeVisible();
  expect(request).toHaveBeenCalledTimes(1);
  again.unmount();
  const failing = vi.fn(async () => {
    throw new Error("offline");
  });
  render(card(failing));
  await waitFor(() => expect(failing).toHaveBeenCalled());
  expect(screen.queryByText("Moves money")).toBeNull();
  expect(screen.queryByRole("note")).toBeNull();
});
it("links incomplete declarations to editors and agent tests without asserting execution", () => {
  const nav = vi.fn(),
    assist = vi.fn();
  render(
    <LocaleScope locale="en-GB">
      <AppDiagnostics
        manifest={template()}
        onNavigate={nav}
        onAssist={assist}
      />
    </LocaleScope>,
  );
  expect(
    screen.getByText(/package declarations, not test results/),
  ).toBeVisible();
  fireEvent.click(screen.getByRole("button", { name: /Required settings/ }));
  expect(nav).toHaveBeenCalledWith("settings");
  fireEvent.click(screen.getByRole("button", { name: /Prepare a test plan/ }));
  expect(assist).toHaveBeenCalledWith(
    expect.stringContaining("denied, retry and rollback"),
  );
});
it("keeps actual read-only review blocked without mislabelling accepted or completed work", () => {
  const approve = vi.fn();
  const message: any = {
    id: 1,
    applied: false,
    data: {
      preview: {
        catalogBefore: [],
        proposal: {
          changes: [],
          operations: [
            {
              name: "merchant.category.create",
              label: "Create category",
              arguments_json: "{}",
            },
          ],
        },
      },
    },
  };
  const view = (applied = false, approvalStarted = false) => (
    <LocaleScope locale="en-GB">
      <ProposalCard
        message={{ ...message, applied }}
        onApply={approve}
        busy={false}
        canApply={false}
        approvalStarted={approvalStarted}
      />
    </LocaleScope>
  );
  const { rerender } = render(view());
  expect(screen.getByText(/read.only/i)).toBeVisible();
  expect(screen.getByRole("button", { name: /approve/i })).toBeDisabled();
  rerender(view(false, true));
  expect(screen.queryByText(/read.only/i)).toBeNull();
  expect(screen.queryByRole("button", { name: /approve/i })).toBeNull();
  rerender(view(true));
  expect(screen.queryByText(/read.only/i)).toBeNull();
  expect(screen.queryByRole("button", { name: /approve/i })).toBeNull();
  expect(approve).not.toHaveBeenCalled();
});
it("compares stable block identity, theme changes and real responsive preview entry", () => {
  const before = { ...newPage("default", "en-GB", "Home"), revision: 2 };
  before.content.blocks = [newBlock("text", "en-GB", "Old")];
  const after = {
    ...before,
    content: { blocks: [newBlock("form", "en-GB", "Contact")] },
    theme: { accent: "#ffffff" },
  };
  const preview = vi.fn();
  render(
    <LocaleScope locale="en-GB">
      <DesignReview before={before} after={after} onPreview={preview} />
    </LocaleScope>,
  );
  expect(
    screen.getByText("Elements removed").nextElementSibling,
  ).toHaveTextContent("1");
  expect(
    screen.getByText("Forms in proposal").nextElementSibling,
  ).toHaveTextContent("1");
  fireEvent.click(screen.getByRole("button", { name: "Preview mobile" }));
  expect(preview).toHaveBeenCalledWith("mobile");
  expect(screen.getByText(/do not certify/)).toBeVisible();
});
