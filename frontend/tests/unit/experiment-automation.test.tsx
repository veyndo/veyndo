/** Experiment suggestion, automation switches and decision note: nothing is saved by a suggestion and applying needs finishing. */
import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { expect, it, vi } from "vitest";
import {
  AutomationFields,
  AutomationNote,
  ExperimentSuggestion,
} from "../../src/admin/intelligence/ExperimentAutomation";
import { LocaleProvider } from "../../src/shared/i18n/i18n";
import type { RequestFn } from "../../src/admin/shell/studio-types";
it("prefills the form from the suggestion, names its basis and warns when no live payments exist", async () => {
  const onUse = vi.fn();
  const request = vi.fn(async () => ({
    design: {
      title: "discovery vs comparison",
      control: "discovery",
      durationHours: 336,
      automation: { finish: true, apply: false },
    },
    basis: {
      livePayments30Days: 0,
      p95PaymentMinor: 0,
      currentMode: "balanced",
      currency: "EUR",
      scale: 2,
    },
    warnings: ["no_live_payments", "future_code"],
  }));
  render(
    <LocaleProvider>
      <ExperimentSuggestion
        request={request as unknown as RequestFn}
        disabled={false}
        onUse={onUse}
      />
    </LocaleProvider>,
  );
  fireEvent.click(screen.getByRole("button"));
  await waitFor(() => expect(onUse).toHaveBeenCalledTimes(1));
  expect(request).toHaveBeenCalledWith(
    "/api/intelligence/experiment.suggest",
    {},
    "POST",
  );
  expect(onUse.mock.calls[0][0]).toMatchObject({
    durationHours: 336,
    automation: { finish: true, apply: false },
  });
  expect(onUse.mock.calls[0][0].title).not.toBe("discovery vs comparison");
  expect(document.querySelectorAll(".experiment-warning")).toHaveLength(1);
  expect(screen.getByRole("status").textContent).toContain("balanced");
});
it("only allows adopting a proved layout together with finishing by itself", () => {
  const onChange = vi.fn();
  const view = render(
    <LocaleProvider>
      <AutomationFields value={undefined} onChange={onChange} />
    </LocaleProvider>,
  );
  const [finish, apply] = screen.getAllByRole("checkbox") as HTMLInputElement[];
  expect(apply.disabled).toBe(true);
  fireEvent.click(finish);
  expect(onChange).toHaveBeenLastCalledWith({ finish: true, apply: false });
  view.rerender(
    <LocaleProvider>
      <AutomationFields
        value={{ finish: true, apply: true }}
        onChange={onChange}
      />
    </LocaleProvider>,
  );
  fireEvent.click(screen.getAllByRole("checkbox")[0]);
  expect(onChange).toHaveBeenLastCalledWith({ finish: false, apply: false });
});
it("shows the automation badges and the decision, and nothing for a manual experiment", () => {
  const view = render(
    <LocaleProvider>
      <AutomationNote automation={undefined} decision={undefined} />
    </LocaleProvider>,
  );
  expect(document.querySelector(".experiment-automation-note")).toBeNull();
  view.rerender(
    <LocaleProvider>
      <AutomationNote
        automation={{ finish: true, apply: true }}
        decision="published"
      />
    </LocaleProvider>,
  );
  expect(
    document.querySelectorAll(".experiment-automation-note span"),
  ).toHaveLength(2);
  expect(
    document.querySelector(".experiment-automation-note strong"),
  ).toBeTruthy();
});
