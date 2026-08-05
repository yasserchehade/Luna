export type TodayNavigationKey =
  | "Today"
  | "Conversations"
  | "Calendar"
  | "Cabinet"
  | "Household"
  | "History"
  | "Settings";

export type HouseholdWorkStatus =
  | "needsAttention"
  | "awaitingApproval"
  | "needsClarification"
  | "upcoming"
  | "completed"
  | "dismissed";

export type ConversationEntry = {
  id: string;
  speaker: "member" | "luna";
  message: string;
};

export type HouseholdFactView = {
  key: string;
  label: string;
  value: string;
};

export type ProposedActionView = {
  id: string;
  label: string;
  description: string;
  approvalRequired: boolean;
};

export type HouseholdWorkView = {
  id: string;
  title: string;
  summary: string;
  status: HouseholdWorkStatus;
  source: {
    label: string;
    detail: string;
  };
  dueLabel: string;
  amountLabel?: string;
  householdEntity: string;
  activity: string;
  recommendation: string;
  needs: string | null;
  facts: HouseholdFactView[];
  proposedAction?: ProposedActionView;
  conversation: ConversationEntry[];
};

export type TodayBriefing = {
  member: {
    displayName: string;
    householdName: string;
    initials: string;
  };
  dateLabel: string;
  greeting: string;
  reviewed: {
    emails: number;
    documents: number;
    calendar: boolean;
  };
  conversation: ConversationEntry[];
  work: HouseholdWorkView[];
  partialFailures: Array<{
    id: string;
    title: string;
    message: string;
  }>;
};

export type ConversationInput = {
  message: string;
  workId?: string;
  attachmentId?: string;
};

export type FactCorrectionInput = {
  workId: string;
  factKey: string;
  value: string;
};

export type MutationResult = {
  briefing: TodayBriefing;
  work: HouseholdWorkView | null;
  confirmation: string;
};

export type AttachmentResult = {
  attachmentId: string;
  displayName: string;
  sizeLabel: string;
};

export type TodayServiceErrorCode = "unavailable" | "invalidAttachment" | "notFound" | "mutationFailed";

export class TodayServiceError extends Error {
  constructor(
    public readonly code: TodayServiceErrorCode,
    message: string,
  ) {
    super(message);
    this.name = "TodayServiceError";
  }
}

export interface TodayService {
  getBriefing(): Promise<TodayBriefing>;
  getWorkItem(id: string): Promise<HouseholdWorkView>;
  sendMessage(input: ConversationInput): Promise<MutationResult>;
  approveAction(workId: string, actionId: string): Promise<MutationResult>;
  dismissWork(workId: string): Promise<MutationResult>;
  completeWork(workId: string): Promise<MutationResult>;
  correctFact(input: FactCorrectionInput): Promise<MutationResult>;
  attachSource(file: File): Promise<AttachmentResult>;
}
