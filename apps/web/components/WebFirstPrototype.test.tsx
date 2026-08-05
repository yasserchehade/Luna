import { fireEvent, render, screen, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { WebFirstPrototype } from "./WebFirstPrototype";

vi.mock("next/navigation", () => ({
  usePathname: () => "/prototype/web-first",
  useRouter: () => ({ replace: vi.fn() }),
  useSearchParams: () => new URLSearchParams("variant=A"),
}));

describe("web-first Luna prototype", () => {
  beforeEach(() => vi.clearAllMocks());

  it("renders semantic navigation and keeps the delegation composer present", async () => {
    const user = userEvent.setup();
    render(<WebFirstPrototype variant="A" layoutOverride="desktop" />);
    const navigation = screen.getByRole("navigation", { name: "Primary navigation" });
    await user.click(within(navigation).getByRole("button", { name: "Calendar" }));
    expect(within(navigation).getByRole("button", { name: "Calendar" }).getAttribute("aria-current")).toBe("page");
    expect(screen.getByPlaceholderText("What would you like me to take care of?")).toBeTruthy();
  });

  it("updates visible working context when Household Work is selected", async () => {
    const user = userEvent.setup();
    render(<WebFirstPrototype variant="A" layoutOverride="desktop" />);
    await user.click(screen.getByRole("button", { name: "Open Rental insurance renewal" }));
    const context = screen.getByRole("complementary", { name: "Working context" });
    expect(within(context).getByText("Rental insurance renewal")).toBeTruthy();
    expect(within(context).getByText("Harbour Mutual")).toBeTruthy();
  });

  it("supports approval and removes dismissed work from attention", async () => {
    const user = userEvent.setup();
    render(<WebFirstPrototype variant="A" layoutOverride="desktop" />);
    await user.click(screen.getByRole("button", { name: "Approve reminder" }));
    expect(screen.getByRole("status").textContent).toContain("Reminder approved");
    const dismissButtons = screen.getAllByRole("button", { name: "Dismiss" });
    await user.click(dismissButtons[0]);
    expect(screen.getByRole("status").textContent).toContain("dismissed");
  });

  it("supports completion and correction without a backend", async () => {
    const user = userEvent.setup();
    render(<WebFirstPrototype variant="A" layoutOverride="desktop" />);
    await user.click(screen.getByRole("button", { name: "Open Rental insurance renewal" }));
    await user.click(screen.getByRole("button", { name: "Correct details" }));
    const correction = screen.getByLabelText("Correct Luna's understanding");
    await user.clear(correction);
    await user.type(correction, "The excess is $750, not $900.");
    await user.click(screen.getByRole("button", { name: "Save correction" }));
    expect(screen.getAllByText("The excess is $750, not $900.").length).toBeGreaterThan(0);
    await user.click(screen.getAllByRole("button", { name: "Mark complete" })[0]);
    expect(screen.getByRole("status").textContent).toContain("marked complete");
  });

  it("shows an attachment in the persistent composer", async () => {
    const user = userEvent.setup();
    const { container } = render(<WebFirstPrototype variant="C" layoutOverride="desktop" />);
    const input = container.querySelector<HTMLInputElement>('input[type="file"]');
    expect(input).not.toBeNull();
    if (!input) throw new Error("attachment input missing");
    await user.upload(input, new File(["safe fixture"], "electricity-bill.pdf", { type: "application/pdf" }));
    expect(screen.getByText("electricity-bill.pdf")).toBeTruthy();
  });

  it("uses a mobile navigation and details drawer instead of forcing three columns", async () => {
    const user = userEvent.setup();
    const { container } = render(<WebFirstPrototype variant="B" layoutOverride="mobile" />);
    expect(screen.getByRole("navigation", { name: "Mobile navigation" })).toBeTruthy();
    expect(container.querySelector(".web-shell")?.getAttribute("data-layout")).toBe("mobile");
    await user.click(screen.getByRole("button", { name: "Work details" }));
    expect(screen.getByRole("complementary", { name: "Working context" })).toBeTruthy();
    expect(screen.getByRole("button", { name: "Close work details" })).toBeTruthy();
  });

  it.each([
    ["loading", "I’m putting your briefing together…"],
    ["empty", "Everything is taken care of."],
    ["error", "I couldn’t finish your briefing."],
  ] as const)("renders the %s briefing state while retaining the composer", (fixtureState, heading) => {
    render(<WebFirstPrototype variant="A" fixtureState={fixtureState} layoutOverride="desktop" />);
    expect(screen.getByRole("heading", { name: heading })).toBeTruthy();
    expect(screen.getByPlaceholderText("What would you like me to take care of?")).toBeTruthy();
  });
});
