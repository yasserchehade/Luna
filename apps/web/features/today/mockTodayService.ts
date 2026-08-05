import {
  TodayServiceError,
  type AttachmentResult,
  type ConversationInput,
  type ConversationMessage,
  type ConversationResult,
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

  const nextMessage = (
    role: ConversationMessage["role"],
    body: string,
    contextualWorkIds: string[] = [],
  ): ConversationMessage => {
    sequence += 1;
    return {
      id: `${role}-${sequence}`,
      role,
      body,
      createdAt: new Date(Date.UTC(2026, 7, 5, 6, 12, sequence)).toISOString(),
      ...(contextualWorkIds.length > 0 ? { contextualWorkIds: [...contextualWorkIds] } : {}),
    };
  };

  const validContextIds = (ids: string[] | undefined): string[] => (
    [...new Set(ids ?? [])].filter((id) => briefing.work.some((work) => work.id === id))
  );

  const recentSingleWorkId = (): string | null => {
    for (let index = briefing.conversation.length - 1; index >= 0; index -= 1) {
      const ids = briefing.conversation[index].contextualWorkIds ?? [];
      if (ids.length === 1) return ids[0];
    }
    return null;
  };

  const explicitWorkIds = (message: string): string[] => {
    const ids: string[] = [];
    const add = (id: string) => { if (!ids.includes(id)) ids.push(id); };

    if (message.includes("both bills")) {
      add("electricity-bill");
      add("insurance-renewal");
    }
    if (/\b(electricity|northstar|electric bill)\b/.test(message) || (/\bbill\b/.test(message) && !message.includes("both bills"))) add("electricity-bill");
    if (/\b(insurance|renewal|excess|harbour mutual)\b/.test(message)) add("insurance-renewal");
    if (/\b(school|excursion|permission form)\b/.test(message)) add("school-form");
    if (/\b(plumber|plumbing|leaking tap)\b/.test(message)) add("repair-followup");

    return ids;
  };

  const activeWork = () => briefing.work.filter((work) => !["completed", "dismissed"].includes(work.status));

  const attentionSummary = (): string => {
    const titles = activeWork().map((work) => work.title);
    if (titles.length === 0) return "Nothing else needs your attention right now.";
    if (titles.length === 1) return `${titles[0]} still needs attention.`;
    return `${titles.slice(0, -1).join(", ")} and ${titles.at(-1)} still need attention.`;
  };

  const mutate = async (
    workId: string,
    confirmation: string,
    update: (work: HouseholdWorkView) => void,
    conversationResponse = confirmation,
  ): Promise<MutationResult> => {
    await delay();
    if (remainingMutationFailures > 0) {
      remainingMutationFailures -= 1;
      throw new TodayServiceError("mutationFailed", "I could not save that change. Your household work was not changed.");
    }
    const work = findWork(workId);
    update(work);
    briefing.conversation.push(nextMessage("luna", conversationResponse, [workId]));
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

    async sendMessage(input: ConversationInput): Promise<ConversationResult> {
      await delay();
      if (remainingMessageFailures > 0) {
        remainingMessageFailures -= 1;
        throw new TodayServiceError("mutationFailed", "I could not add that instruction. Your draft and household work were not changed.");
      }
      const message = input.message.trim();
      if (!message && !input.attachmentId) {
        throw new TodayServiceError("mutationFailed", "Add an instruction or attachment before sending.");
      }

      const normalizedMessage = message.toLocaleLowerCase();
      const contextIds = validContextIds(input.contextualWorkIds);
      const explicitIds = explicitWorkIds(normalizedMessage);
      const recentId = recentSingleWorkId();
      const asksForAttention = /\bwhat\b.*\b(else|still|needs?)\b.*\b(attention|need)|\bwhat still needs my attention\b/.test(normalizedMessage);
      const paidIntent = /\b(already paid|mark(?:ed)? .* paid|paid (it|that|this)|have paid)\b/.test(normalizedMessage);
      const dismissIntent = /\bdismiss\b/.test(normalizedMessage);
      const rentalCorrection = normalizedMessage.includes("rental property");
      const keepExcess = normalizedMessage.includes("keep the current excess");
      const referential = /\b(it|that|this|current)\b/.test(normalizedMessage);

      let resolvedIds = [...explicitIds];
      if (resolvedIds.length === 0 && rentalCorrection) resolvedIds = ["electricity-bill"];
      if (resolvedIds.length === 0 && referential && contextIds.length === 1) resolvedIds = [...contextIds];
      if (resolvedIds.length === 0 && referential && recentId) resolvedIds = [recentId];

      const memberContextIds = asksForAttention ? [] : resolvedIds.length > 0 ? resolvedIds : contextIds;
      const memberMessage = nextMessage("member", message || "I attached a household document.", memberContextIds);
      briefing.conversation.push(memberMessage);

      let affectedWorkIds: string[] = [];
      let clarification: ConversationResult["clarification"];
      let response: string;
      let responseContextIds: string[] = [];

      if (asksForAttention) {
        response = attentionSummary();
        responseContextIds = activeWork().map((work) => work.id);
      } else if (paidIntent && resolvedIds.length !== 1 && !dismissIntent) {
        clarification = {
          question: "Which item did you pay?",
          candidateWorkIds: activeWork().map((work) => work.id),
        };
        response = clarification.question;
        responseContextIds = clarification.candidateWorkIds ?? [];
      } else {
        const confirmations: string[] = [];

        if (paidIntent) {
          const paidWorkId = resolvedIds.find((id) => id === "electricity-bill") ?? (resolvedIds.length === 1 ? resolvedIds[0] : null);
          if (paidWorkId) {
            const work = findWork(paidWorkId);
            work.status = "completed";
            work.dueLabel = "Completed";
            work.needs = null;
            work.proposedAction = undefined;
            affectedWorkIds.push(work.id);
            confirmations.push(`I marked ${work.title} complete`);
          }
        }

        if (dismissIntent) {
          for (const workId of resolvedIds.filter((id) => id === "school-form" || !paidIntent)) {
            const work = findWork(workId);
            work.status = "dismissed";
            work.needs = null;
            if (!affectedWorkIds.includes(work.id)) affectedWorkIds.push(work.id);
            confirmations.push(`I dismissed ${work.title}`);
          }
        }

        if (rentalCorrection && resolvedIds.includes("electricity-bill")) {
          const work = findWork("electricity-bill");
          const property = work.facts.find((fact) => fact.key === "property");
          if (property) {
            property.value = "Rental property";
            work.householdEntity = "Rental property";
            if (!affectedWorkIds.includes(work.id)) affectedWorkIds.push(work.id);
            confirmations.push("I updated the electricity bill's property to Rental property and kept the other details unchanged");
          }
        }

        if (keepExcess && resolvedIds.includes("insurance-renewal")) {
          const work = findWork("insurance-renewal");
          const excess = work.facts.find((fact) => fact.key === "excess")?.value ?? "current excess";
          work.status = "upcoming";
          work.needs = null;
          work.recommendation = `Keep the ${excess} excess and continue reviewing the renewal.`;
          if (!affectedWorkIds.includes(work.id)) affectedWorkIds.push(work.id);
          confirmations.push(`I kept the ${excess} excess for the insurance renewal`);
        }

        if (paidIntent && affectedWorkIds.length === 1 && confirmations.length === 1) {
          response = `Thanks — I marked this complete and moved ${findWork(affectedWorkIds[0]).title} out of today's attention.`;
        } else if (confirmations.length > 0) {
          response = `${confirmations.join(" and ")}.`;
        } else if (input.attachmentId) {
          response = "I have the document and will keep this instruction with it.";
        } else if (normalizedMessage.includes("both bills") && resolvedIds.length === 2) {
          affectedWorkIds = [...resolvedIds];
          response = "I will keep the electricity bill and insurance renewal in view together. No external action was taken.";
        } else if (resolvedIds.length > 0) {
          affectedWorkIds = [...resolvedIds];
          const titles = resolvedIds.map((id) => findWork(id).title);
          response = `I have added that direction with ${titles.join(" and ")} as relevant context.`;
        } else {
          response = "I have added that instruction to today's conversation.";
        }
        responseContextIds = affectedWorkIds.length > 0 ? affectedWorkIds : resolvedIds;
      }

      const lunaMessage = nextMessage("luna", response, responseContextIds);
      briefing.conversation.push(lunaMessage);

      return {
        briefing: cloneBriefing(briefing),
        memberMessage: structuredClone(memberMessage),
        lunaMessage: structuredClone(lunaMessage),
        affectedWorkIds,
        ...(clarification ? { clarification } : {}),
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
      }, "Reminder scheduled for 12 August. I will keep watching for a payment confirmation.");
    },

    dismissWork(workId) {
      return mutate(workId, "I dismissed that household work.", (work) => {
        work.status = "dismissed";
        work.needs = null;
      });
    },

    completeWork(workId) {
      return mutate(workId, "I marked that household work complete.", (work) => {
        work.status = "completed";
        work.dueLabel = "Completed";
        work.needs = null;
        work.proposedAction = undefined;
      });
    },

    correctFact(input: FactCorrectionInput) {
      return mutate(input.workId, "I corrected that fact and left the rest unchanged.", (work) => {
        const fact = work.facts.find((candidate) => candidate.key === input.factKey);
        if (!fact || !input.value.trim()) {
          throw new TodayServiceError("notFound", "That fact could not be corrected.");
        }
        fact.value = input.value.trim();
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
