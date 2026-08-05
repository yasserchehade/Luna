import {
  TodayServiceError,
  type AttachmentResult,
  type ConversationInput,
  type FactCorrectionInput,
  type HouseholdWorkView,
  type MutationResult,
  type TodayBriefing,
  type TodayService,
} from "./contracts";

const MAX_SOURCE_BYTES = 5 * 1024 * 1024;
const SUPPORTED_SOURCE_TYPES = new Set(["application/pdf", "image/jpeg", "image/png"]);

const fixtureBriefing: TodayBriefing = {
  member: {
    displayName: "Yasser",
    householdName: "Chehade household",
    initials: "YC",
  },
  dateLabel: "Wednesday, 5 August",
  greeting: "Good afternoon",
  reviewed: {
    emails: 24,
    documents: 2,
    calendar: true,
  },
  conversation: [],
  partialFailures: [],
  work: [
    {
      id: "electricity-bill",
      title: "Electricity bill needs approval",
      summary: "Northstar Electricity issued a $184.72 bill for Home, due 15 August.",
      status: "awaitingApproval",
      source: {
        label: "Electricity bill.pdf",
        detail: "Uploaded document · reviewed today at 4:12 pm",
      },
      dueLabel: "Due 15 Aug",
      amountLabel: "$184.72",
      householdEntity: "Home",
      activity: "I matched the service address to Home and checked that no payment confirmation has arrived.",
      recommendation: "Schedule a reminder for three days before the due date.",
      needs: "Your approval to schedule the reminder",
      facts: [
        { key: "provider", label: "Provider", value: "Northstar Electricity" },
        { key: "amount", label: "Amount", value: "$184.72" },
        { key: "account", label: "Account", value: "NS-123456" },
        { key: "property", label: "Property", value: "Home" },
      ],
      proposedAction: {
        id: "reminder-12-august",
        label: "Approve reminder",
        description: "Remind the household on 12 August.",
        approvalRequired: true,
      },
      conversation: [],
    },
    {
      id: "insurance-renewal",
      title: "Rental insurance renewal",
      summary: "The renewal quote is ready, but the excess changed from last year.",
      status: "needsClarification",
      source: {
        label: "Harbour Mutual renewal notice",
        detail: "Household source · reviewed today at 3:48 pm",
      },
      dueLabel: "Due 19 Aug",
      amountLabel: "$1,248 yearly",
      householdEntity: "Rental property",
      activity: "I compared the renewal with last year's policy and found one material change.",
      recommendation: "Review the higher excess before accepting the renewal.",
      needs: "Whether the higher excess is acceptable",
      facts: [
        { key: "provider", label: "Provider", value: "Harbour Mutual" },
        { key: "premium", label: "Premium", value: "$1,248 yearly" },
        { key: "excess", label: "New excess", value: "$900" },
        { key: "previous-excess", label: "Previous excess", value: "$650" },
      ],
      conversation: [
        {
          id: "insurance-question",
          speaker: "luna",
          message: "The premium is close to last year, but the excess increased by $250. Is that acceptable?",
        },
      ],
    },
    {
      id: "school-form",
      title: "School excursion form prepared",
      summary: "The permission form is complete and ready for your signature.",
      status: "upcoming",
      source: {
        label: "School portal notice",
        detail: "Household source · reviewed yesterday",
      },
      dueLabel: "Due 22 Aug",
      amountLabel: "$18",
      householdEntity: "Amira",
      activity: "I filled the known emergency contact and dietary information from the household record.",
      recommendation: "Sign before Friday; no other information is missing.",
      needs: "Your signature by Friday",
      facts: [
        { key: "event", label: "Event", value: "Science museum excursion" },
        { key: "date", label: "Date", value: "28 August" },
        { key: "cost", label: "Cost", value: "$18" },
        { key: "member", label: "Household member", value: "Amira" },
      ],
      conversation: [],
    },
    {
      id: "repair-followup",
      title: "Plumber follow-up sent",
      summary: "I followed up about the leaking tap. The plumber confirmed Tuesday morning.",
      status: "completed",
      source: {
        label: "Conversation with Bayside Plumbing",
        detail: "Completed action · today at 9:16 am",
      },
      dueLabel: "Completed",
      householdEntity: "Home",
      activity: "I sent the approved follow-up and recorded the confirmed visit window.",
      recommendation: "No action needed unless Tuesday no longer works.",
      needs: null,
      facts: [
        { key: "provider", label: "Provider", value: "Bayside Plumbing" },
        { key: "visit", label: "Visit", value: "Tuesday, 9–11 am" },
      ],
      conversation: [],
    },
  ],
};

function cloneBriefing(briefing: TodayBriefing): TodayBriefing {
  return structuredClone(briefing);
}

function sizeLabel(bytes: number): string {
  if (bytes < 1024) return `${bytes} B`;
  if (bytes < 1024 * 1024) return `${Math.ceil(bytes / 1024)} KB`;
  return `${(bytes / (1024 * 1024)).toFixed(1)} MB`;
}

export type MockTodayServiceOptions = {
  initialState?: "ready" | "empty";
  latencyMs?: number;
  loadFailures?: number;
  unavailable?: boolean;
  partialFailure?: boolean;
  mutationFailures?: number;
  messageFailures?: number;
};

export function createMockTodayService(options: MockTodayServiceOptions = {}): TodayService {
  let briefing = cloneBriefing(fixtureBriefing);
  let remainingLoadFailures = options.loadFailures ?? 0;
  let remainingMutationFailures = options.mutationFailures ?? 0;
  let remainingMessageFailures = options.messageFailures ?? 0;
  let sequence = 0;

  if (options.initialState === "empty") briefing.work = [];
  if (options.partialFailure) {
    briefing.partialFailures = [{
      id: "calendar-unavailable",
      title: "Calendar review is unavailable",
      message: "I could not check upcoming calendar items. The household work below is still current.",
    }];
  }

  const delay = async () => {
    const latency = options.latencyMs ?? 20;
    if (latency > 0) await new Promise((resolve) => setTimeout(resolve, latency));
  };

  const findWork = (workId: string): HouseholdWorkView => {
    const work = briefing.work.find((candidate) => candidate.id === workId);
    if (!work) throw new TodayServiceError("notFound", "That household work is no longer available.");
    return work;
  };

  const mutate = async (
    workId: string,
    confirmation: string,
    update: (work: HouseholdWorkView) => void,
  ): Promise<MutationResult> => {
    await delay();
    if (remainingMutationFailures > 0) {
      remainingMutationFailures -= 1;
      throw new TodayServiceError("mutationFailed", "I could not save that change. Your household work was not changed.");
    }
    const work = findWork(workId);
    update(work);
    return { briefing: cloneBriefing(briefing), work: structuredClone(work), confirmation };
  };

  return {
    async getBriefing() {
      await delay();
      if (options.unavailable) {
        throw new TodayServiceError("unavailable", "Luna is temporarily unavailable. Your household work is safe.");
      }
      if (remainingLoadFailures > 0) {
        remainingLoadFailures -= 1;
        throw new TodayServiceError("mutationFailed", "I could not load today's household work. Your records were not changed.");
      }
      return cloneBriefing(briefing);
    },

    async getWorkItem(id) {
      await delay();
      return structuredClone(findWork(id));
    },

    async sendMessage(input: ConversationInput) {
      await delay();
      if (remainingMessageFailures > 0) {
        remainingMessageFailures -= 1;
        throw new TodayServiceError("mutationFailed", "I could not add that instruction. Your draft and household work were not changed.");
      }
      const message = input.message.trim();
      if (!message && !input.attachmentId) {
        throw new TodayServiceError("mutationFailed", "Add an instruction or attachment before sending.");
      }

      const work = input.workId ? findWork(input.workId) : null;
      const confirmation = input.attachmentId
        ? "I have the document and will keep this instruction with it."
        : work
          ? `I have added that direction to ${work.title}.`
          : "I have added that instruction to today's conversation.";

      if (work) {
        if (message) work.conversation.push({ id: `member-${++sequence}`, speaker: "member", message });
        work.conversation.push({ id: `luna-${++sequence}`, speaker: "luna", message: confirmation });
      } else {
        if (message) briefing.conversation.push({ id: `member-${++sequence}`, speaker: "member", message });
        briefing.conversation.push({ id: `luna-${++sequence}`, speaker: "luna", message: confirmation });
      }

      return {
        briefing: cloneBriefing(briefing),
        work: work ? structuredClone(work) : null,
        confirmation,
      };
    },

    approveAction(workId, actionId) {
      return mutate(workId, "Reminder approved for 12 August.", (work) => {
        if (work.proposedAction?.id !== actionId) {
          throw new TodayServiceError("notFound", "That proposed action is no longer available.");
        }
        work.status = "upcoming";
        work.needs = null;
        work.recommendation = "Reminder scheduled for 12 August. I will keep watching for a payment confirmation.";
        work.proposedAction = undefined;
        work.conversation.push({ id: `luna-${++sequence}`, speaker: "luna", message: "Reminder approved. I will keep this bill in view until it is resolved." });
      });
    },

    dismissWork(workId) {
      return mutate(workId, "I dismissed that household work.", (work) => {
        work.status = "dismissed";
        work.needs = null;
        work.conversation.push({ id: `luna-${++sequence}`, speaker: "luna", message: "Understood. I dismissed this and removed it from today's attention." });
      });
    },

    completeWork(workId) {
      return mutate(workId, "I marked that household work complete.", (work) => {
        work.status = "completed";
        work.dueLabel = "Completed";
        work.needs = null;
        work.proposedAction = undefined;
        work.conversation.push({ id: `luna-${++sequence}`, speaker: "luna", message: "Done. I marked this complete and moved it out of attention." });
      });
    },

    correctFact(input: FactCorrectionInput) {
      return mutate(input.workId, "I corrected that fact and left the rest unchanged.", (work) => {
        const fact = work.facts.find((candidate) => candidate.key === input.factKey);
        if (!fact || !input.value.trim()) {
          throw new TodayServiceError("notFound", "That fact could not be corrected.");
        }
        fact.value = input.value.trim();
        work.conversation.push({
          id: `luna-${++sequence}`,
          speaker: "luna",
          message: `I corrected ${fact.label.toLowerCase()} to ${fact.value} and kept the other details unchanged.`,
        });
      });
    },

    async attachSource(file: File): Promise<AttachmentResult> {
      await delay();
      if (!SUPPORTED_SOURCE_TYPES.has(file.type)) {
        throw new TodayServiceError("invalidAttachment", "Choose a PDF, JPG or PNG household document.");
      }
      if (file.size > MAX_SOURCE_BYTES) {
        throw new TodayServiceError("invalidAttachment", "That document is larger than the 5 MB source limit.");
      }
      return {
        attachmentId: `local-${++sequence}`,
        displayName: file.name,
        sizeLabel: sizeLabel(file.size),
      };
    },
  };
}
