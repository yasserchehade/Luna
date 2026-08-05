import { fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { beforeEach, describe, expect, it, vi } from "vitest";
import type { ConversationInput, ConversationResult, TodayService } from "./contracts";
import { createMockTodayService } from "./mockTodayService";
import { TodayRoute } from "./TodayRoute";

function renderToday(options: Parameters<typeof createMockTodayService>[0] = {}, layout: "mobile" | "tablet" | "desktop" = "desktop") {
  const width = { mobile: 390, tablet: 900, desktop: 1440 }[layout];
  Object.defineProperty(window, "innerWidth", { configurable: true, writable: true, value: width });
  const service = createMockTodayService({ latencyMs: 0, ...options });
  return { service, ...render(<TodayRoute service={service} />) };
}

async function ready() {
  return screen.findByRole("heading", { name: "Good afternoon, Yasser." });
}

function delayedMessageService() {
  const base = createMockTodayService({ latencyMs: 0 });
  let release: (() => void) | null = null;
  const gate = new Promise<void>((resolve) => { release = resolve; });
  const sendMessage = vi.fn(async (input: ConversationInput): Promise<ConversationResult> => {
    await gate;
    return base.sendMessage(input);
  });
  return {
    service: { ...base, sendMessage },
    sendMessage,
    release: () => release?.(),
  };
}

describe("production Today route", () => {
  beforeEach(() => {
    window.localStorage.clear();
    Object.defineProperty(window, "innerWidth", { configurable: true, writable: true, value: 1440 });
  });

  it("renders the proactive Variant A briefing with production navigation and persistent delegation", async () => {
    renderToday();
    await ready();
    expect(screen.getByText(/While you were away, I reviewed/)).toBeTruthy();
    expect(screen.getByText("Needs your attention")).toBeTruthy();
    expect(screen.getByRole("navigation", { name: "Primary navigation" })).toBeTruthy();
    expect(screen.queryByText("Dashboard")).toBeNull();
    expect(screen.getByRole("form", { name: "Delegate to Luna" })).toBeTruthy();
    expect(screen.queryByText(/Variant B|Variant C/)).toBeNull();
  });

  it("keeps compact navigation destinations accessibly named", async () => {
    renderToday({}, "tablet");
    await ready();
    const navigation = screen.getByRole("navigation", { name: "Primary navigation" });
    expect(within(navigation).getByRole("button", { name: "Today" })).toBeTruthy();
    expect(within(navigation).getByRole("button", { name: "Settings" })).toBeTruthy();
  });

  it("updates working context when household work is selected", async () => {
    const user = userEvent.setup();
    renderToday();
    await ready();
    await user.click(screen.getByRole("button", { name: "Open Rental insurance renewal" }));
    const context = screen.getByRole("complementary", { name: "Working context" });
    expect(within(context).getByText("Rental insurance renewal")).toBeTruthy();
    expect(within(context).getByText("Harbour Mutual")).toBeTruthy();
  });

  it("opens with a global composer and no selected or contextual work", async () => {
    renderToday();
    await ready();

    expect(screen.getByPlaceholderText("What would you like me to take care of?")).toBeTruthy();
    expect(screen.queryByRole("button", { name: /Remove .* conversation context/ })).toBeNull();
    expect(screen.getByRole("complementary", { name: "Working context" }).textContent).toContain("Nothing selected");
  });

  it("completes explicitly referenced work without selecting a report and keeps the exchange global", async () => {
    const user = userEvent.setup();
    renderToday();
    await ready();
    const composer = screen.getByLabelText("Instruction for Luna");

    await user.type(composer, "I already paid the electricity bill.{enter}");

    const conversation = await screen.findByRole("log", { name: "Household conversation" });
    expect(within(conversation).getByText("I already paid the electricity bill.")).toBeTruthy();
    expect(within(conversation).getByText(/marked this complete/)).toBeTruthy();
    expect(screen.getByRole("complementary", { name: "Working context" }).textContent).toContain("Nothing selected");
  });

  it("removes optional conversation context without clearing the draft or viewed work", async () => {
    const user = userEvent.setup();
    renderToday();
    await ready();
    await user.click(screen.getByRole("button", { name: "Open Rental insurance renewal" }));
    const composer = screen.getByLabelText("Instruction for Luna") as HTMLTextAreaElement;
    await user.type(composer, "Keep the current excess");

    await user.click(screen.getByRole("button", { name: "Remove Rental insurance renewal from conversation context" }));

    expect(composer.value).toBe("Keep the current excess");
    expect(screen.getByRole("complementary", { name: "Working context" }).textContent).toContain("Rental insurance renewal");
    expect(screen.getByPlaceholderText("What would you like me to take care of?")).toBeTruthy();
  });

  it("keeps one conversation visible while viewing different reports", async () => {
    const user = userEvent.setup();
    renderToday();
    await ready();
    await user.type(screen.getByLabelText("Instruction for Luna"), "What still needs my attention?{enter}");
    const conversation = await screen.findByRole("log", { name: "Household conversation" });
    const conversationText = conversation.textContent;

    await user.click(screen.getByRole("button", { name: "Open Rental insurance renewal" }));
    await user.click(screen.getByRole("button", { name: "Open School excursion form prepared" }));

    expect(screen.getByRole("log", { name: "Household conversation" }).textContent).toBe(conversationText);
    expect(screen.queryByLabelText(/Conversation about/)).toBeNull();
  });

  it("uses selected insurance as an optional hint and updates that work in the global conversation", async () => {
    const user = userEvent.setup();
    renderToday();
    await ready();
    await user.click(screen.getByRole("button", { name: "Open Rental insurance renewal" }));

    await user.type(screen.getByLabelText("Instruction for Luna"), "Keep the current excess.{enter}");

    const conversation = await screen.findByRole("log", { name: "Household conversation" });
    expect(within(conversation).getByText("Keep the current excess.")).toBeTruthy();
    expect(within(conversation).getByText(/kept the \$900 excess/)).toBeTruthy();
    const context = screen.getByRole("complementary", { name: "Working context" });
    expect(within(context).getByText("Rental insurance renewal")).toBeTruthy();
    expect(within(context).getByText("Keep the $900 excess and continue reviewing the renewal.")).toBeTruthy();
  });

  it("answers a global attention question even while electricity work is selected", async () => {
    const user = userEvent.setup();
    renderToday();
    await ready();
    await user.click(screen.getByRole("button", { name: "Open Electricity bill needs approval" }));

    await user.type(screen.getByLabelText("Instruction for Luna"), "What else needs attention?{enter}");

    const conversation = await screen.findByRole("log", { name: "Household conversation" });
    const response = within(conversation).getByText(/still need attention/);
    expect(response.textContent).toContain("Rental insurance renewal");
    expect(response.textContent).toContain("School excursion form prepared");
    expect(screen.getByRole("complementary", { name: "Working context" }).textContent).toContain("Electricity bill needs approval");
  });

  it("asks for clarification without mutating work when an unscoped reference is ambiguous", async () => {
    const user = userEvent.setup();
    renderToday();
    await ready();

    await user.type(screen.getByLabelText("Instruction for Luna"), "I already paid it.{enter}");

    const conversation = await screen.findByRole("log", { name: "Household conversation" });
    expect(within(conversation).getByText("Which item did you pay?")).toBeTruthy();
    expect(screen.getByRole("button", { name: "Approve reminder" })).toBeTruthy();
    expect(screen.getByRole("button", { name: "Open School excursion form prepared" })).toBeTruthy();
  });

  it("approves a proposed action and reports the optimistic transition", async () => {
    const user = userEvent.setup();
    renderToday();
    await ready();
    await user.click(screen.getByRole("button", { name: "Approve reminder" }));
    expect((await screen.findByRole("status")).textContent).toContain("Reminder approved");
    expect(screen.queryByRole("button", { name: "Approve reminder" })).toBeNull();
    expect(screen.getByText("Reminder scheduled for 12 August. I will keep watching for a payment confirmation.")).toBeTruthy();
  });

  it("retries a failed mocked mutation without losing the work", async () => {
    const user = userEvent.setup();
    renderToday({ mutationFailures: 1 });
    await ready();
    await user.click(screen.getByRole("button", { name: "Approve reminder" }));
    const alert = await screen.findByRole("alert");
    expect(alert.textContent).toContain("Your household work was not changed");
    await user.click(within(alert).getByRole("button", { name: "Retry" }));
    expect((await screen.findByRole("status")).textContent).toContain("Reminder approved");
  });

  it("corrects one visible fact while preserving the rest", async () => {
    const user = userEvent.setup();
    renderToday();
    await ready();
    await user.click(screen.getByRole("button", { name: "Open Rental insurance renewal" }));
    await user.click(screen.getByRole("button", { name: "Correct a fact" }));
    await user.selectOptions(screen.getByLabelText("Fact to correct"), "excess");
    const value = screen.getByLabelText("Correct value");
    await user.clear(value);
    await user.type(value, "$750");
    await user.click(screen.getByRole("button", { name: "Save correction" }));
    await waitFor(() => expect(screen.getAllByText("$750").length).toBeGreaterThan(0));
    expect(screen.getAllByText("$1,248 yearly").length).toBeGreaterThan(0);
  });

  it("dismisses work from attention and completes work into the completed section", async () => {
    const user = userEvent.setup();
    renderToday();
    await ready();

    const insurance = screen.getByRole("button", { name: "Open Rental insurance renewal" }).closest("article");
    if (!insurance) throw new Error("insurance report missing");
    await user.click(within(insurance).getByRole("button", { name: "Dismiss" }));
    await waitFor(() => expect(screen.queryByRole("button", { name: "Open Rental insurance renewal" })).toBeNull());

    const electricity = screen.getByRole("button", { name: "Open Electricity bill needs approval" }).closest("article");
    if (!electricity) throw new Error("electricity report missing");
    await user.click(within(electricity).getByRole("button", { name: "Mark complete" }));
    expect((await screen.findByRole("status")).textContent).toContain("marked that household work complete");
    expect(screen.getByText("Completed while you were away")).toBeTruthy();
  });

  it("selects an attachment through the bounded mock adapter", async () => {
    const user = userEvent.setup();
    const { container } = renderToday();
    await ready();
    const input = container.querySelector<HTMLInputElement>('input[type="file"]');
    if (!input) throw new Error("attachment control missing");
    await user.upload(input, new File(["safe fixture"], "northstar-bill.pdf", { type: "application/pdf" }));
    expect(await screen.findByRole("button", { name: "Remove northstar-bill.pdf" })).toBeTruthy();
    expect(screen.getByText(/Nothing has been uploaded/)).toBeTruthy();
  });

  it("sends a contextual instruction with Enter and preserves Shift+Enter for multiline text", async () => {
    const user = userEvent.setup();
    renderToday();
    await ready();
    const composer = screen.getByLabelText("Instruction for Luna");
    await user.type(composer, "Keep this open{shift>}{enter}{/shift}until I pay it");
    expect((composer as HTMLTextAreaElement).value).toBe("Keep this open\nuntil I pay it");
    await user.type(composer, "{enter}");
    expect(await screen.findByText((_, element) => element?.tagName === "P" && element.textContent === "Keep this open\nuntil I pay it")).toBeTruthy();
    expect((composer as HTMLTextAreaElement).value).toBe("");
  });

  it("submits with the send button and restores focus after Luna responds", async () => {
    const user = userEvent.setup();
    renderToday();
    await ready();
    const composer = screen.getByLabelText("Instruction for Luna") as HTMLTextAreaElement;
    await user.type(composer, "Keep watching for the receipt");
    await user.click(screen.getByRole("button", { name: "Send instruction" }));
    expect(await screen.findByText("Keep watching for the receipt")).toBeTruthy();
    expect(screen.getAllByText("I have added that instruction to today's conversation.").length).toBeGreaterThan(0);
    expect(composer.value).toBe("");
    expect(document.activeElement).toBe(composer);
  });

  it("renders the member message optimistically and prevents duplicate pending sends", async () => {
    const user = userEvent.setup();
    const delayed = delayedMessageService();
    render(<TodayRoute service={delayed.service} />);
    await ready();
    const composer = screen.getByLabelText("Instruction for Luna") as HTMLTextAreaElement;
    const form = screen.getByRole("form", { name: "Delegate to Luna" });
    await user.type(composer, "Please keep this in view");
    fireEvent.submit(form);
    fireEvent.submit(form);

    expect(document.querySelector('[data-pending="true"]')?.textContent).toContain("Please keep this in view");
    expect(delayed.sendMessage).toHaveBeenCalledTimes(1);
    expect(screen.getByRole("button", { name: "Send instruction" }).hasAttribute("disabled")).toBe(true);
    expect(composer.value).toBe("Please keep this in view");

    delayed.release();
    expect((await screen.findAllByText("I have added that instruction to today's conversation.")).length).toBeGreaterThan(0);
    expect(composer.value).toBe("");
    expect(document.activeElement).toBe(composer);
  });

  it("completes selected work from an existing conversational direction without resetting context", async () => {
    const user = userEvent.setup();
    renderToday();
    await ready();
    await user.click(screen.getByRole("button", { name: "Open Electricity bill needs approval" }));
    const composer = screen.getByLabelText("Instruction for Luna");
    await user.type(composer, "I already paid it.");
    await user.click(screen.getByRole("button", { name: "Send instruction" }));

    expect((await screen.findByRole("status")).textContent).toContain("marked this complete");
    const completed = screen.getByText("Completed while you were away").closest("section");
    if (!completed) throw new Error("completed section missing");
    expect(within(completed).getByRole("button", { name: "Open Electricity bill needs approval" })).toBeTruthy();
    const context = screen.getByRole("complementary", { name: "Working context" });
    expect(within(context).getByText("Electricity bill needs approval")).toBeTruthy();
    expect(screen.getAllByText("I already paid it.")).toHaveLength(1);
  });

  it("applies an existing fact correction through conversation without duplicating work", async () => {
    const user = userEvent.setup();
    renderToday();
    await ready();
    const composer = screen.getByLabelText("Instruction for Luna");
    await user.type(composer, "That's for the rental property.");
    await user.click(screen.getByRole("button", { name: "Send instruction" }));

    await user.click(screen.getByRole("button", { name: "Open Electricity bill needs approval" }));
    const context = screen.getByRole("complementary", { name: "Working context" });
    await waitFor(() => expect(within(context).getAllByText("Rental property").length).toBeGreaterThan(0));
    expect(screen.getAllByRole("button", { name: "Open Electricity bill needs approval" })).toHaveLength(1);
    expect(screen.getAllByText("That's for the rental property.")).toHaveLength(1);
  });

  it("sends an existing mocked attachment with a message and clears it only on success", async () => {
    const user = userEvent.setup();
    const { container } = renderToday();
    await ready();
    const input = container.querySelector<HTMLInputElement>('input[type="file"]');
    if (!input) throw new Error("attachment control missing");
    await user.upload(input, new File(["safe fixture"], "northstar-bill.pdf", { type: "application/pdf" }));
    const composer = screen.getByLabelText("Instruction for Luna");
    await user.type(composer, "Take care of this.");
    await user.click(screen.getByRole("button", { name: "Send instruction" }));

    expect((await screen.findAllByText("I have the document and will keep this instruction with it.")).length).toBeGreaterThan(0);
    expect(screen.getByText("Take care of this.")).toBeTruthy();
    expect(screen.queryByRole("button", { name: "Remove northstar-bill.pdf" })).toBeNull();
  });

  it("sends a new delegation outside selected work and keeps it in today's stream", async () => {
    const user = userEvent.setup();
    renderToday();
    await ready();
    const composer = screen.getByLabelText("Instruction for Luna");
    await user.type(composer, "Arrange a locksmith visit{enter}");
    expect(await screen.findByText("Arrange a locksmith visit")).toBeTruthy();
    expect(screen.getAllByText("I have added that instruction to today's conversation.")).toHaveLength(2);
  });

  it("retries a failed new instruction without clearing the draft", async () => {
    const user = userEvent.setup();
    renderToday({ messageFailures: 1 });
    await ready();
    const composer = screen.getByLabelText("Instruction for Luna") as HTMLTextAreaElement;
    await user.type(composer, "Arrange a locksmith visit{enter}");
    const alert = await screen.findByRole("alert");
    expect(alert.textContent).toContain("Your draft and household work were not changed");
    expect(composer.value).toBe("Arrange a locksmith visit");
    await user.click(within(alert).getByRole("button", { name: "Retry" }));
    expect(await screen.findByText("Arrange a locksmith visit")).toBeTruthy();
    expect(screen.getAllByText("Arrange a locksmith visit")).toHaveLength(1);
  });

  it("keeps a draft while navigating between mocked destinations", async () => {
    const user = userEvent.setup();
    renderToday();
    await ready();
    const composer = screen.getByLabelText("Instruction for Luna");
    await user.type(composer, "Ask the insurer for options");
    await user.click(screen.getByRole("button", { name: "Calendar" }));
    expect(screen.getByRole("heading", { name: "Calendar" })).toBeTruthy();
    await user.click(screen.getByRole("button", { name: "Today" }));
    expect((screen.getByLabelText("Instruction for Luna") as HTMLTextAreaElement).value).toBe("Ask the insurer for options");
  });

  it("shows a structured loading state while retaining the composer", () => {
    const base = createMockTodayService({ latencyMs: 0 });
    const service: TodayService = { ...base, getBriefing: () => new Promise(() => undefined) };
    render(<TodayRoute service={service} />);
    expect(screen.getByRole("status", { name: "Loading today's briefing" })).toBeTruthy();
    expect(screen.getByRole("form", { name: "Delegate to Luna" })).toBeTruthy();
  });

  it("speaks naturally when no work needs attention", async () => {
    renderToday({ initialState: "empty" });
    expect(await screen.findByRole("heading", { name: "Everything is under control." })).toBeTruthy();
    expect(screen.getByRole("form", { name: "Delegate to Luna" })).toBeTruthy();
  });

  it("keeps a new instruction visible when the briefing starts empty", async () => {
    const user = userEvent.setup();
    renderToday({ initialState: "empty" });
    await screen.findByRole("heading", { name: "Everything is under control." });
    const composer = screen.getByLabelText("Instruction for Luna");
    await user.type(composer, "Arrange the annual smoke alarm check{enter}");
    expect(await screen.findByText("Arrange the annual smoke alarm check")).toBeTruthy();
    expect(screen.getByRole("heading", { name: "Everything is under control." })).toBeTruthy();
  });

  it("shows a recoverable load error and succeeds on retry", async () => {
    const user = userEvent.setup();
    renderToday({ loadFailures: 1 });
    const alert = await screen.findByRole("alert");
    expect(alert.textContent).toContain("Your records were not changed");
    await user.click(screen.getByRole("button", { name: "Try again" }));
    await ready();
  });

  it("distinguishes service unavailability from an empty briefing", async () => {
    renderToday({ unavailable: true });
    expect((await screen.findByRole("alert")).textContent).toContain("temporarily unavailable");
    expect(screen.queryByText("Everything is under control.")).toBeNull();
  });

  it("renders available work when one briefing source fails", async () => {
    renderToday({ partialFailure: true });
    await ready();
    expect(screen.getByText("Calendar review is unavailable")).toBeTruthy();
    expect(screen.getByRole("button", { name: "Open Electricity bill needs approval" })).toBeTruthy();
  });

  it("uses a keyboard-accessible context drawer on mobile", async () => {
    const user = userEvent.setup();
    const { container } = renderToday({}, "mobile");
    await ready();
    expect(container.querySelector(".today-shell")?.getAttribute("data-layout")).toBe("mobile");
    expect(screen.getByRole("navigation", { name: "Mobile navigation" })).toBeTruthy();
    const opener = screen.getByRole("button", { name: "Work details" });
    await user.click(opener);
    expect(screen.getByRole("dialog", { name: "Working context" })).toBeTruthy();
    expect(document.activeElement).toBe(screen.getByRole("button", { name: "Close work details" }));
    await user.keyboard("{Escape}");
    expect(screen.queryByRole("button", { name: "Close work details" })).toBeNull();
    expect(document.activeElement).toBe(opener);
  });

  it("provides every navigation destination from the mobile menu", async () => {
    const user = userEvent.setup();
    renderToday({}, "mobile");
    await ready();
    const opener = screen.getByRole("button", { name: "Open navigation" });
    await user.click(opener);
    const menu = screen.getByRole("dialog", { name: "All navigation" });
    expect(document.activeElement).toBe(within(menu).getByRole("button", { name: "Close navigation" }));
    expect(within(menu).getByRole("button", { name: "History" })).toBeTruthy();
    expect(within(menu).getByRole("button", { name: "Settings" })).toBeTruthy();
    await user.keyboard("{Escape}");
    expect(screen.queryByRole("dialog", { name: "All navigation" })).toBeNull();
    expect(document.activeElement).toBe(opener);
  });

  it("rejects unsupported attachments with a safe accessible error", async () => {
    const { container } = renderToday();
    await ready();
    const input = container.querySelector<HTMLInputElement>('input[type="file"]');
    if (!input) throw new Error("attachment control missing");
    fireEvent.change(input, { target: { files: [new File(["fixture"], "notes.txt", { type: "text/plain" })] } });
    expect((await screen.findByRole("alert")).textContent).toContain("Choose a PDF, JPG or PNG");
  });
});
