export type PrototypeVariantKey = "A" | "B" | "C";
export type PrototypeFixtureState = "ready" | "loading" | "empty" | "error";
export type WorkStatus = "attention" | "awaitingApproval" | "upcoming" | "completed" | "dismissed";
export type NavigationKey = "Today" | "Conversations" | "Calendar" | "Cabinet" | "Household" | "History" | "Settings";

export type HouseholdWork = {
  id: string;
  title: string;
  summary: string;
  source: string;
  sourceDetail: string;
  due: string;
  entity: string;
  status: WorkStatus;
  recommendation: string;
  facts: Array<{ label: string; value: string }>;
  needs: string | null;
  activity: string;
};

export type PrototypeState = {
  activeNavigation: NavigationKey;
  selectedWorkId: string | null;
  works: HouseholdWork[];
  composer: string;
  attachmentName: string | null;
  correctionOpen: boolean;
  contextOpen: boolean;
  notice: string | null;
};

export type PrototypeAction =
  | { type: "navigate"; destination: NavigationKey }
  | { type: "selectWork"; workId: string }
  | { type: "approve"; workId: string }
  | { type: "dismiss"; workId: string }
  | { type: "complete"; workId: string }
  | { type: "discuss"; workId: string }
  | { type: "openCorrection"; workId: string }
  | { type: "cancelCorrection" }
  | { type: "saveCorrection"; workId: string; value: string }
  | { type: "setComposer"; value: string }
  | { type: "attach"; filename: string }
  | { type: "toggleContext"; open: boolean }
  | { type: "send" };

export const initialWorks: HouseholdWork[] = [
  {
    id: "electricity-bill",
    title: "Electricity bill needs approval",
    summary: "Northstar Electricity issued a $184.72 bill for Home, due 15 August.",
    source: "Electricity bill.pdf",
    sourceDetail: "Uploaded document · reviewed today at 4:12 pm",
    due: "15 Aug",
    entity: "Home",
    status: "awaitingApproval",
    recommendation: "Approve a reminder for three days before the due date.",
    facts: [
      { label: "Provider", value: "Northstar Electricity" },
      { label: "Amount", value: "$184.72" },
      { label: "Account", value: "NS-123456" },
      { label: "Property", value: "Home" },
    ],
    needs: "Your approval to schedule the reminder",
    activity: "I matched the service address to Home and checked that no payment confirmation has arrived.",
  },
  {
    id: "insurance-renewal",
    title: "Rental insurance renewal",
    summary: "The renewal quote is ready, but the excess changed from last year.",
    source: "Harbour Mutual renewal notice",
    sourceDetail: "Email fixture · reviewed today at 3:48 pm",
    due: "19 Aug",
    entity: "Rental property",
    status: "attention",
    recommendation: "Review the higher excess before accepting the renewal.",
    facts: [
      { label: "Provider", value: "Harbour Mutual" },
      { label: "Premium", value: "$1,248 yearly" },
      { label: "New excess", value: "$900" },
      { label: "Previous excess", value: "$650" },
    ],
    needs: "Whether the higher excess is acceptable",
    activity: "I compared the renewal with last year's policy and found one material change.",
  },
  {
    id: "school-form",
    title: "School excursion form prepared",
    summary: "The permission form is complete and ready for your signature.",
    source: "School portal notice",
    sourceDetail: "Message fixture · reviewed yesterday",
    due: "22 Aug",
    entity: "Amira",
    status: "upcoming",
    recommendation: "Sign before Friday; no other information is missing.",
    facts: [
      { label: "Event", value: "Science museum excursion" },
      { label: "Date", value: "28 August" },
      { label: "Cost", value: "$18" },
      { label: "Member", value: "Amira" },
    ],
    needs: "Your signature by Friday",
    activity: "I filled the known emergency contact and dietary information from the household record.",
  },
  {
    id: "repair-followup",
    title: "Plumber follow-up sent",
    summary: "I followed up about the leaking tap. The plumber confirmed Tuesday morning.",
    source: "Conversation with Bayside Plumbing",
    sourceDetail: "Mock completed action · today at 9:16 am",
    due: "Completed",
    entity: "Home",
    status: "completed",
    recommendation: "No action needed unless Tuesday no longer works.",
    facts: [
      { label: "Provider", value: "Bayside Plumbing" },
      { label: "Visit", value: "Tuesday, 9–11 am" },
    ],
    needs: null,
    activity: "I sent the approved follow-up and recorded the confirmed visit window.",
  },
];

export function createInitialState(): PrototypeState {
  return {
    activeNavigation: "Today",
    selectedWorkId: initialWorks[0].id,
    works: initialWorks.map((work) => ({ ...work, facts: work.facts.map((fact) => ({ ...fact })) })),
    composer: "",
    attachmentName: null,
    correctionOpen: false,
    contextOpen: false,
    notice: null,
  };
}

function updateWork(state: PrototypeState, workId: string, update: Partial<HouseholdWork>): PrototypeState {
  return {
    ...state,
    works: state.works.map((work) => (work.id === workId ? { ...work, ...update } : work)),
  };
}

export function prototypeReducer(state: PrototypeState, action: PrototypeAction): PrototypeState {
  switch (action.type) {
    case "navigate":
      return { ...state, activeNavigation: action.destination, notice: `${action.destination} is represented by fixture content in this prototype.` };
    case "selectWork":
      return { ...state, selectedWorkId: action.workId, contextOpen: true, correctionOpen: false, notice: null };
    case "approve":
      return {
        ...updateWork(state, action.workId, {
          status: "upcoming",
          needs: null,
          recommendation: "Reminder approved for 12 August. I’ll keep watching for a payment confirmation.",
        }),
        selectedWorkId: action.workId,
        notice: "Reminder approved. This mock action changed local prototype state only.",
      };
    case "dismiss":
      return {
        ...updateWork(state, action.workId, { status: "dismissed", needs: null }),
        notice: "Household Work dismissed in local mock state.",
      };
    case "complete":
      return {
        ...updateWork(state, action.workId, { status: "completed", due: "Completed", needs: null }),
        notice: "Household Work marked complete in local mock state.",
      };
    case "discuss": {
      const work = state.works.find((candidate) => candidate.id === action.workId);
      return {
        ...state,
        selectedWorkId: action.workId,
        composer: work ? `Let's discuss ${work.title.toLowerCase()}. ` : state.composer,
        notice: null,
      };
    }
    case "openCorrection":
      return { ...state, selectedWorkId: action.workId, correctionOpen: true, notice: null };
    case "cancelCorrection":
      return { ...state, correctionOpen: false };
    case "saveCorrection": {
      const work = state.works.find((candidate) => candidate.id === action.workId);
      if (!work || !action.value.trim()) return state;
      return {
        ...updateWork(state, action.workId, {
          summary: action.value.trim(),
          activity: "You corrected this work in conversation. Luna would validate and retain the correction as an auditable member direction.",
        }),
        correctionOpen: false,
        notice: "Correction applied to local mock state.",
      };
    }
    case "setComposer":
      return { ...state, composer: action.value };
    case "attach":
      return { ...state, attachmentName: action.filename, notice: `${action.filename} attached locally. Nothing was uploaded.` };
    case "toggleContext":
      return { ...state, contextOpen: action.open };
    case "send":
      if (!state.composer.trim() && !state.attachmentName) return state;
      return { ...state, composer: "", attachmentName: null, notice: "Message added to the mock conversation. No backend was contacted." };
  }
}

export function layoutModeForWidth(width: number): "mobile" | "tablet" | "desktop" {
  if (width < 720) return "mobile";
  if (width < 1120) return "tablet";
  return "desktop";
}

export function visibleAttention(works: HouseholdWork[]): HouseholdWork[] {
  return works.filter((work) => work.status === "attention" || work.status === "awaitingApproval");
}
