import { BillActions } from "../components/BillActions";
import { CabinetActions } from "../components/CabinetActions";
import { CreateMenu } from "../components/CreateMenu";
import { GraphLinkActions } from "../components/GraphLinkActions";
import { StructureEditor } from "../components/StructureEditor";
import { ApprovalActions } from "../components/ApprovalActions";
import { UploadDocumentForm } from "../components/UploadDocumentForm";

type Bill = {
  id: string;
  document_id?: string | null;
  supplier: string;
  supplier_entity_id?: string | null;
  amount?: number | null;
  due_date?: string | null;
  invoice_number?: string;
  category?: string;
  classification?: string;
  status: "draft" | "unpaid" | "paid" | "overdue" | "archived";
  extraction_confidence?: number | null;
  review_status: "not_required" | "needs_review" | "confirmed";
  review_reasons: string[];
};

type HouseholdEntity = {
  id: string;
  entity_type: string;
  display_name: string;
};

type HouseholdGraphNode = {
  id: string;
  node_type: string;
  display_name: string;
  metadata?: Record<string, unknown>;
};

type EntityRelationship = {
  id: string;
  source_entity_type: string;
  source_entity_id: string;
  relationship_type: string;
  target_entity_type: string;
  target_entity_id: string;
};

type HouseholdGraph = {
  nodes: HouseholdGraphNode[];
  relationships: EntityRelationship[];
};

type GraphSuggestion = {
  id: string;
  confidence: number;
  suggested_action: string;
  reasoning: string;
  affected_entities: { display_name?: string; entity_id?: string; entity_type?: string }[];
  status: string;
  action_payload: Record<string, unknown>;
  source_document_id?: string | null;
  source_bill_id?: string | null;
};

type HouseholdTask = {
  id: string;
  title: string;
  description?: string | null;
  due_date?: string | null;
};

type HouseholdReminder = {
  id: string;
  title: string;
  remind_at: string;
};

type HouseholdObligation = {
  id: string;
  source_bill_id?: string | null;
  title: string;
  supplier?: string | null;
  amount?: number | null;
  currency: string;
  due_date?: string | null;
  status: "needs_review" | "upcoming" | "due_soon" | "overdue" | "paid" | "archived";
  evidence: Record<string, unknown>;
};

type HouseholdSummary = {
  entities: HouseholdEntity[];
  open_tasks: HouseholdTask[];
  upcoming_reminders: HouseholdReminder[];
  upcoming_obligations: HouseholdObligation[];
  overdue_obligations: HouseholdObligation[];
  needs_review_obligations: HouseholdObligation[];
};

type DocumentRecord = {
  id: string;
  original_filename: string;
  cabinet_status: "unplanned" | "suggested" | "confirmed" | "filed" | "needs_review";
  suggested_cabinet_path?: string | null;
  confirmed_cabinet_path?: string | null;
};

type DocumentSearchResult = {
  document: DocumentRecord;
  supplier?: string | null;
  invoice_number?: string | null;
  category?: string | null;
  snippet?: string | null;
};

type KnowledgeAnswer = {
  question: string;
  answer: string;
  confidence: number;
  sources: {
    source_type: string;
    source_id: string;
    title: string;
    detail?: string | null;
  }[];
  suggested_next_actions: string[];
};

type ApprovalRequest = {
  id: string;
  work_order_id: string;
  status: "pending" | "approved" | "rejected" | "dismissed" | "escalated";
  requested_approver_role?: "owner" | "admin" | "member" | "viewer" | null;
  reason: string;
  decision_reason?: string | null;
};

type WorkOrder = {
  id: string;
  work_type: string;
  title: string;
  description?: string | null;
  status:
    | "observed"
    | "prepared"
    | "proposed"
    | "approval_requested"
    | "approved"
    | "executed"
    | "escalated"
    | "rejected"
    | "dismissed";
  capability_required: "read" | "write" | "execute";
  subject_entity_type?: string | null;
  subject_entity_id?: string | null;
  source_document_id?: string | null;
  source_bill_id?: string | null;
  evidence: Record<string, unknown>;
  result: Record<string, unknown>;
};

type AuditEvent = {
  id: string;
  event_type: string;
  entity_type?: string | null;
  entity_id?: string | null;
  metadata: Record<string, unknown>;
  created_at: string;
};

type ActiveTab =
  | "dashboard"
  | "cabinet"
  | "bills"
  | "structure"
  | "approvals"
  | "assistant"
  | "audit"
  | "settings";

const NAV_ITEMS: { href: string; icon: string; label: string; tab: ActiveTab }[] = [
  { href: "/", icon: "⌂", label: "Today", tab: "dashboard" },
  { href: "/?tab=cabinet", icon: "▤", label: "Documents", tab: "cabinet" },
  { href: "/?tab=bills", icon: "◇", label: "Bills & renewals", tab: "bills" },
  { href: "/?tab=structure", icon: "◎", label: "Household", tab: "structure" },
  { href: "/?tab=assistant", icon: "✦", label: "Ask Luna", tab: "assistant" },
  { href: "/?tab=audit", icon: "↺", label: "History", tab: "audit" },
  { href: "/?tab=settings", icon: "⚙", label: "Settings", tab: "settings" },
];

const PAGE_META: Record<ActiveTab, { title: string; subtitle: string }> = {
  dashboard: {
    title: "Today",
    subtitle: "A quiet overview of what needs attention at home.",
  },
  cabinet: {
    title: "Documents",
    subtitle: "Everything important, filed and easy to find.",
  },
  bills: {
    title: "Bills & renewals",
    subtitle: "Upcoming costs and dates, without the spreadsheet feeling.",
  },
  structure: {
    title: "Household",
    subtitle: "Members, properties, vehicles, suppliers, policies, accounts, and relationships.",
  },
  approvals: {
    title: "Approvals",
    subtitle: "Work Luna has prepared and is waiting for an authorised decision.",
  },
  assistant: {
    title: "Ask Luna",
    subtitle: "Ask a question about your home, documents, or upcoming commitments.",
  },
  audit: {
    title: "History",
    subtitle: "A clear record of what changed, when, and why.",
  },
  settings: {
    title: "Settings",
    subtitle: "A future home for authority, storage, connections, and household preferences.",
  },
};

function getApiBaseUrl() {
  return (
    process.env.API_INTERNAL_BASE_URL ??
    process.env.NEXT_PUBLIC_API_BASE_URL ??
    "http://localhost:8000"
  );
}

async function getBills(): Promise<Bill[]> {
  try {
    const response = await fetch(`${getApiBaseUrl()}/api/bills`, { cache: "no-store" });
    if (!response.ok) {
      return [];
    }
    return response.json();
  } catch {
    return [];
  }
}

async function getHouseholdSummary(): Promise<HouseholdSummary> {
  try {
    const response = await fetch(`${getApiBaseUrl()}/api/household/summary`, {
      cache: "no-store",
    });
    if (!response.ok) {
      return emptyHouseholdSummary();
    }
    return response.json();
  } catch {
    return emptyHouseholdSummary();
  }
}

function emptyHouseholdSummary(): HouseholdSummary {
  return {
    entities: [],
    open_tasks: [],
    overdue_obligations: [],
    upcoming_obligations: [],
    upcoming_reminders: [],
    needs_review_obligations: [],
  };
}

async function getHouseholdGraph(): Promise<HouseholdGraph> {
  try {
    const response = await fetch(`${getApiBaseUrl()}/api/household/graph`, {
      cache: "no-store",
    });
    if (!response.ok) {
      return { nodes: [], relationships: [] };
    }
    return response.json();
  } catch {
    return { nodes: [], relationships: [] };
  }
}

async function getGraphSuggestions(): Promise<GraphSuggestion[]> {
  try {
    const response = await fetch(`${getApiBaseUrl()}/api/household/suggestions`, {
      cache: "no-store",
    });
    if (!response.ok) {
      return [];
    }
    const payload = (await response.json()) as { suggestions: GraphSuggestion[] };
    return payload.suggestions;
  } catch {
    return [];
  }
}

async function getDocuments(): Promise<DocumentRecord[]> {
  try {
    const response = await fetch(`${getApiBaseUrl()}/api/documents`, {
      cache: "no-store",
    });
    if (!response.ok) {
      return [];
    }
    return response.json();
  } catch {
    return [];
  }
}

async function searchDocuments(query: string): Promise<DocumentSearchResult[]> {
  if (query.trim().length < 2) {
    return [];
  }

  try {
    const params = new URLSearchParams({ query });
    const response = await fetch(`${getApiBaseUrl()}/api/documents/search?${params}`, {
      cache: "no-store",
    });
    if (!response.ok) {
      return [];
    }
    return response.json();
  } catch {
    return [];
  }
}

async function askKnowledge(question: string): Promise<KnowledgeAnswer | null> {
  if (question.trim().length < 2) {
    return null;
  }

  try {
    const response = await fetch(`${getApiBaseUrl()}/api/knowledge/ask`, {
      body: JSON.stringify({ question }),
      cache: "no-store",
      headers: { "Content-Type": "application/json" },
      method: "POST",
    });
    if (!response.ok) {
      return null;
    }
    return response.json();
  } catch {
    return null;
  }
}

async function getApprovalRequests(): Promise<ApprovalRequest[]> {
  try {
    const response = await fetch(`${getApiBaseUrl()}/api/work/approvals`, {
      cache: "no-store",
    });
    if (!response.ok) {
      return [];
    }
    return response.json();
  } catch {
    return [];
  }
}

async function getWorkOrders(): Promise<WorkOrder[]> {
  try {
    const response = await fetch(`${getApiBaseUrl()}/api/work/orders`, {
      cache: "no-store",
    });
    if (!response.ok) {
      return [];
    }
    return response.json();
  } catch {
    return [];
  }
}

async function getAuditEvents(): Promise<AuditEvent[]> {
  try {
    const response = await fetch(`${getApiBaseUrl()}/api/audit-events?limit=40`, {
      cache: "no-store",
    });
    if (!response.ok) {
      return [];
    }
    return response.json();
  } catch {
    return [];
  }
}

function normalizeTab(tab?: string | string[]): ActiveTab {
  const value = Array.isArray(tab) ? tab[0] : tab;
  if (
    value === "cabinet" ||
    value === "bills" ||
    value === "structure" ||
    value === "approvals" ||
    value === "assistant" ||
    value === "audit" ||
    value === "settings"
  ) {
    return value;
  }
  return "dashboard";
}

function cabinetPath(document: DocumentRecord) {
  return document.confirmed_cabinet_path ?? document.suggested_cabinet_path;
}

function normalizedLabel(value: string) {
  return value.replaceAll("_", " ").replaceAll(".", " ");
}

function reminderTiming(value: string) {
  const today = new Date();
  const target = new Date(value);
  today.setHours(0, 0, 0, 0);
  target.setHours(0, 0, 0, 0);
  const days = Math.round((target.getTime() - today.getTime()) / 86400000);
  if (days < 0) return `Overdue by ${Math.abs(days)} day${Math.abs(days) === 1 ? "" : "s"}`;
  if (days === 0) return "Due today";
  if (days === 1) return "Due tomorrow";
  return `Due in ${days} days`;
}

function reminderTone(value: string) {
  const today = new Date();
  const target = new Date(value);
  today.setHours(0, 0, 0, 0);
  target.setHours(0, 0, 0, 0);
  const days = Math.round((target.getTime() - today.getTime()) / 86400000);
  if (days < 0) return "overdueReminder";
  if (days <= 1) return "soonReminder";
  return "upcomingReminder";
}

function obligationDueLabel(obligation: HouseholdObligation) {
  if (!obligation.due_date) {
    return "Due date needs review";
  }
  return reminderTiming(obligation.due_date);
}

function obligationAmount(obligation: HouseholdObligation) {
  if (obligation.amount == null) {
    return "Amount pending";
  }
  return `${obligation.currency} ${obligation.amount.toFixed(2)}`;
}

function sourceLabel(workOrder?: WorkOrder) {
  if (!workOrder) {
    return "Source work not loaded";
  }
  if (workOrder.source_bill_id) {
    return `Bill ${workOrder.source_bill_id}`;
  }
  if (workOrder.source_document_id) {
    return `Document ${workOrder.source_document_id}`;
  }
  if (workOrder.subject_entity_type && workOrder.subject_entity_id) {
    return `${workOrder.subject_entity_type} ${workOrder.subject_entity_id}`;
  }
  return "Internal Luna work";
}

function evidenceEntries(evidence?: Record<string, unknown>) {
  if (!evidence) {
    return [];
  }
  return Object.entries(evidence)
    .filter(([, value]) => value !== null && value !== undefined && value !== "")
    .slice(0, 5);
}

function shortValue(value: unknown) {
  if (Array.isArray(value)) {
    return value.join(", ");
  }
  if (typeof value === "object" && value !== null) {
    return JSON.stringify(value);
  }
  return String(value);
}

function auditLabel(event: AuditEvent) {
  const metadata = event.metadata ?? {};
  switch (event.event_type) {
    case "work_order.created":
      return `Luna prepared ${String(metadata.work_type ?? "work")}`;
    case "approval_request.created":
      return "Approval requested";
    case "approval_request.approved":
      return "User approved Luna work";
    case "approval_request.rejected":
      return "User rejected Luna work";
    case "approval_request.dismissed":
      return "User dismissed Luna work";
    case "obligation.created":
      return "Obligation created";
    case "obligation.updated":
      return "Obligation updated";
    case "obligation.status_changed":
      return "Obligation status changed";
    case "reminder.created":
      return "Reminder created";
    case "document.cabinet_path_confirmed":
      return "Document filing approved";
    case "document.cabinet_plan_not_approved":
      return `Document filing ${String(metadata.approval_decision ?? "not approved")}`;
    case "bill.ingested":
      return "Luna prepared bill";
    case "bill.confirmed":
      return "Bill confirmed";
    case "document.uploaded":
      return "Household record uploaded";
    case "household_entity.created":
      return "Household record created";
    case "relationship.created":
      return "Household relationship recorded";
    case "graph_suggestion.accepted":
      return "Luna suggestion accepted";
    case "knowledge.question_answered":
      return "Luna answered a question";
    default:
      return normalizedLabel(event.event_type);
  }
}

function auditKind(event: AuditEvent) {
  const type = normalizedLabel(event.event_type);
  return `Activity type: ${type}`;
}

function auditRecordLabel(event: AuditEvent) {
  const parts = [
    event.entity_type ? normalizedLabel(event.entity_type) : null,
    event.entity_id ?? null,
  ].filter(Boolean);
  return parts.length > 0 ? parts.join(" - ") : "Household activity";
}

function auditDetail(event: AuditEvent) {
  const parts = [
    event.entity_type ? normalizedLabel(event.entity_type) : null,
    event.entity_id ?? null,
  ].filter(Boolean);
  return parts.length > 0 ? parts.join(" · ") : "Household activity";
}

type DemoStepStatus = "Not started" | "Ready" | "Waiting for approval" | "Complete";

function demoStepClass(status: DemoStepStatus) {
  return status.toLowerCase().replaceAll(" ", "-");
}

export default async function LunaWorkbenchPage({
  searchParams,
}: {
  searchParams?: Promise<{
    ask?: string | string[];
    mode?: string | string[];
    q?: string | string[];
    tab?: string | string[];
  }>;
}) {
  const resolvedSearchParams = await searchParams;
  const activeTab = normalizeTab(resolvedSearchParams?.tab);
  const cabinetQuery = Array.isArray(resolvedSearchParams?.q)
    ? resolvedSearchParams?.q[0] ?? ""
    : resolvedSearchParams?.q ?? "";
  const assistantQuestion = Array.isArray(resolvedSearchParams?.ask)
    ? resolvedSearchParams?.ask[0] ?? ""
    : resolvedSearchParams?.ask ?? "";
  const navigatorModeParam = Array.isArray(resolvedSearchParams?.mode)
    ? resolvedSearchParams?.mode[0]
    : resolvedSearchParams?.mode;
  const navigatorInitialMode =
    navigatorModeParam === "add"
      ? "createEntity"
      : navigatorModeParam === "link"
        ? "createRelationship"
        : undefined;
  const [
    bills,
    household,
    householdGraph,
    documents,
    graphSuggestions,
    approvalRequests,
    workOrders,
    auditEvents,
  ] = await Promise.all([
    getBills(),
    getHouseholdSummary(),
    getHouseholdGraph(),
    getDocuments(),
    getGraphSuggestions(),
    getApprovalRequests(),
    getWorkOrders(),
    getAuditEvents(),
  ]);
  const searchResults =
    activeTab === "cabinet" && cabinetQuery.trim().length >= 2
      ? await searchDocuments(cabinetQuery)
      : [];
  const assistantAnswer =
    activeTab === "assistant" && assistantQuestion.trim().length >= 2
      ? await askKnowledge(assistantQuestion)
      : null;
  const cabinetDocuments =
    searchResults.length > 0
      ? searchResults.map((result) => result.document)
      : documents;
  const graphNodesById = new Map(householdGraph.nodes.map((node) => [node.id, node]));
  const workOrdersById = new Map(workOrders.map((workOrder) => [workOrder.id, workOrder]));
  const billsById = new Map(bills.map((bill) => [bill.id, bill]));
  const documentsById = new Map(documents.map((document) => [document.id, document]));

  const unpaid = bills.filter((bill) => bill.status === "unpaid");
  const overdue = household.overdue_obligations;
  const paid = bills.filter((bill) => bill.status === "paid");
  const needsReview = household.needs_review_obligations;
  const monitoredObligations = [
    ...household.overdue_obligations,
    ...household.upcoming_obligations,
    ...household.needs_review_obligations,
  ];
  const pendingApprovals = approvalRequests.filter((approval) => approval.status === "pending");
  const preparedWork = workOrders.filter((workOrder) =>
    ["observed", "prepared", "proposed", "approval_requested"].includes(workOrder.status),
  );
  const recentlyCompletedWork = workOrders.filter((workOrder) =>
    ["approved", "executed", "rejected", "dismissed"].includes(workOrder.status),
  );
  const cabinetSuggestions = documents.filter(
    (document) => document.cabinet_status === "suggested",
  );
  const confirmedDocuments = documents.filter(
    (document) => document.cabinet_status === "confirmed",
  );
  const cabinetNeedsReview = documents.filter(
    (document) =>
      document.cabinet_status === "unplanned" || document.cabinet_status === "needs_review",
  );
  const hasRecord = documents.length > 0 || bills.length > 0;
  const hasPreparedWork = workOrders.length > 0;
  const hasPendingApproval = pendingApprovals.length > 0;
  const hasDecision = approvalRequests.some((approval) => approval.status !== "pending");
  const hasObligation = monitoredObligations.length > 0;
  const hasActivity = auditEvents.length > 0;
  const demoSteps: { label: string; status: DemoStepStatus }[] = [
    {
      label: "Add a household record.",
      status: hasRecord ? "Complete" : "Ready",
    },
    {
      label: "Ask Luna to review it.",
      status: hasPreparedWork ? "Complete" : hasRecord ? "Ready" : "Not started",
    },
    {
      label: "Luna prepares work.",
      status: hasPreparedWork ? "Complete" : hasRecord ? "Ready" : "Not started",
    },
    {
      label: "Approve or dismiss Luna's request.",
      status: hasPendingApproval
        ? "Waiting for approval"
        : hasDecision
          ? "Complete"
          : hasPreparedWork
            ? "Ready"
            : "Not started",
    },
    {
      label: "Luna monitors the obligation.",
      status: hasObligation ? "Complete" : hasDecision ? "Ready" : "Not started",
    },
    {
      label: "Review Luna's activity trail.",
      status: hasActivity ? "Complete" : "Not started",
    },
  ];
  const pageMeta = PAGE_META[activeTab];
  const attentionCount =
    pendingApprovals.length + household.open_tasks.length + overdue.length + needsReview.length;
  const dueTotal = [...household.overdue_obligations, ...household.upcoming_obligations]
    .reduce((total, obligation) => total + (obligation.amount ?? 0), 0);
  const dateLabel = new Intl.DateTimeFormat("en-AU", {
    day: "numeric",
    month: "long",
    weekday: "long",
  }).format(new Date());
  return (
    <main className="appShell">
      <aside className="sidebar" aria-label="Luna navigation">
        <div className="brandBlock">
          <span className="brandMark">L</span>
          <div>
            <strong>Luna</strong>
            <span>Household companion</span>
          </div>
        </div>

        <nav className="sidebarNav">
          {NAV_ITEMS.map((item) => (
            <a
              className={activeTab === item.tab ? "activeNavItem" : ""}
              href={item.href}
              key={item.tab}
            >
              <span>{item.icon}</span>
              {item.label}
            </a>
          ))}
        </nav>

        <div className="sidebarUser">
          <span>Y</span>
          <div>
            <strong>Your household</strong>
            <small>Private workspace</small>
          </div>
        </div>
      </aside>

      <section className="mainContent">
        <section className="pageHeader">
          <div>
            <p className="eyebrow">{dateLabel}</p>
            <h1>{pageMeta.title}</h1>
            <span>{pageMeta.subtitle}</span>
          </div>
          <div className="pageHeaderActions">
            <a className="assistantButton" href="/?tab=assistant">
              <span>✦</span> Ask Luna
            </a>
            {activeTab !== "dashboard" ? <CreateMenu /> : null}
          </div>
        </section>

      {activeTab === "dashboard" ? (
        <>
          <section className="todayHero">
            <div className="heroCopy">
              <span className="heroKicker">HOME AT A GLANCE</span>
              <h2>{attentionCount > 0 ? `${attentionCount} things need you` : "Everything looks settled"}</h2>
              <p>
                {attentionCount > 0
                  ? "Luna has gathered the important items so you can deal with them in one pass."
                  : "There is nothing urgent right now. Luna will keep watching the details in the background."}
              </p>
              <div className="heroActions">
                <CreateMenu />
                <a href="/?tab=assistant">Ask about my household <span>→</span></a>
              </div>
            </div>
            <div className="calmStatus" aria-label={`${attentionCount} items need attention`}>
              <span className="statusHalo"><i>{attentionCount}</i></span>
              <strong>{attentionCount === 0 ? "All clear" : "To review"}</strong>
              <small>Luna is watching {documents.length + monitoredObligations.length} records</small>
            </div>
          </section>

          <section className="glanceStats" aria-label="Household summary">
            <div>
              <span className="statIcon warm">◇</span>
              <p><strong>{monitoredObligations.length}</strong><span>Active commitments</span></p>
              <small>{dueTotal > 0 ? `$${dueTotal.toFixed(2)} tracked` : "Nothing due yet"}</small>
            </div>
            <div>
              <span className="statIcon violet">✓</span>
              <p><strong>{pendingApprovals.length}</strong><span>Waiting for you</span></p>
              <small>{pendingApprovals.length === 0 ? "No decisions pending" : "Ready to review"}</small>
            </div>
            <div>
              <span className="statIcon mint">▤</span>
              <p><strong>{documents.length}</strong><span>Documents</span></p>
              <small>{confirmedDocuments.length} neatly filed</small>
            </div>
          </section>

          <section className="todayGrid">
            <div className="attentionPanel">
              <div className="sectionHeading">
                <div>
                  <span className="sectionEyebrow">YOUR DAY</span>
                  <h2>Needs your attention</h2>
                </div>
                <span className="countBadge">{attentionCount}</span>
              </div>

              <div className="attentionList">
                {pendingApprovals.slice(0, 3).map((approval) => (
                  <article className="attentionItem" key={approval.id}>
                    <span className="itemMarker violet">✓</span>
                    <div>
                      <span className="itemType">Approval</span>
                      <strong>{approval.reason}</strong>
                      <small>{sourceLabel(workOrdersById.get(approval.work_order_id))}</small>
                    </div>
                    <ApprovalActions approvalId={approval.id} status={approval.status} />
                  </article>
                ))}
                {household.open_tasks.slice(0, 3).map((task) => (
                  <article className="attentionItem" key={task.id}>
                    <span className="itemMarker warm">!</span>
                    <div>
                      <span className="itemType">To review</span>
                      <strong>{task.title}</strong>
                      <small>{task.description ?? "Luna needs a quick check from you."}</small>
                    </div>
                    <a className="textAction" href="/?tab=assistant">Review →</a>
                  </article>
                ))}
                {overdue.slice(0, 3).map((obligation) => (
                  <article className="attentionItem" key={obligation.id}>
                    <span className="itemMarker coral">◇</span>
                    <div>
                      <span className="itemType">Overdue</span>
                      <strong>{obligation.title}</strong>
                      <small>{obligationAmount(obligation)} · {obligationDueLabel(obligation)}</small>
                    </div>
                    <a className="textAction" href="/?tab=bills">View →</a>
                  </article>
                ))}
                {attentionCount === 0 ? (
                  <div className="clearState">
                    <span>✓</span>
                    <div>
                      <strong>You are all caught up</strong>
                      <p>New decisions, dates, or documents that need a look will appear here.</p>
                    </div>
                  </div>
                ) : null}
              </div>
              <a className="panelFooterLink" href="/?tab=audit">See everything Luna has done <span>→</span></a>
            </div>

            <aside className="todaySide">
              <section className="lunaNote">
                <span className="lunaOrb">✦</span>
                <span className="sectionEyebrow">FROM LUNA</span>
                <h2>{hasRecord ? "Your household is taking shape" : "Start with one small thing"}</h2>
                <p>
                  {hasRecord
                    ? `I’m keeping an eye on ${documents.length} document${documents.length === 1 ? "" : "s"} and ${monitoredObligations.length} commitment${monitoredObligations.length === 1 ? "" : "s"}.`
                    : "Add a bill, policy, or household document. I’ll pull out the important details and keep them organised."}
                </p>
                <a href="/?tab=assistant">Talk to Luna <span>→</span></a>
              </section>

              <section className="recentPanel">
                <div className="sectionHeading compact">
                  <h2>Recently</h2>
                  <a href="/?tab=audit">View all</a>
                </div>
                <div className="recentList">
                  {auditEvents.slice(0, 4).map((event) => (
                    <div key={event.id}>
                      <span className="timelineDot" />
                      <p><strong>{auditLabel(event)}</strong><small>{new Date(event.created_at).toLocaleDateString("en-AU", { day: "numeric", month: "short" })}</small></p>
                    </div>
                  ))}
                  {auditEvents.length === 0 ? <p className="mutedCopy">Luna’s recent work will show here.</p> : null}
                </div>
              </section>
            </aside>
          </section>

          <div className="legacyDashboard" aria-hidden="true">
          <section className="demoWalkthrough" aria-label="Demo 1 walkthrough">
            <div>
              <p className="eyebrow">Demo 1</p>
              <h2>Luna's first day at work</h2>
              <span>
                Follow the path from household record to prepared work, approval,
                obligation monitoring, and activity history.
              </span>
            </div>
            <ol>
              {demoSteps.map((step, index) => (
                <li key={step.label}>
                  <span className="stepNumber">{index + 1}</span>
                  <strong>{step.label}</strong>
                  <em className={`demoStepStatus ${demoStepClass(step.status)}`}>
                    {step.status}
                  </em>
                </li>
              ))}
            </ol>
          </section>

          <section className="metrics" aria-label="Luna workbench summary">
            <div>
              <span>Unpaid</span>
              <strong>{unpaid.length}</strong>
            </div>
            <div>
              <span>Overdue obligations</span>
              <strong>{overdue.length}</strong>
            </div>
            <div>
              <span>Needs approval</span>
              <strong>{pendingApprovals.length}</strong>
            </div>
            <div>
              <span>Prepared work</span>
              <strong>{preparedWork.length}</strong>
            </div>
            <div>
              <span>Needs review</span>
              <strong>{needsReview.length}</strong>
            </div>
          </section>

          <section className="operationalGrid" aria-label="Household intelligence summary">
            <div className="panel">
              <div className="panelHeader">
                <h2>Needs approval</h2>
                <span>{pendingApprovals.length} requests</span>
              </div>
              <div className="taskList">
                {pendingApprovals.length === 0 ? (
                  <p className="emptyState">Approval requests will appear when Luna needs authority to continue.</p>
                ) : (
                  pendingApprovals.slice(0, 5).map((approval) => (
                    <div key={approval.id} className="taskRow">
                      <strong>{approval.reason}</strong>
                      <span>{approval.requested_approver_role ?? "authorised household member"}</span>
                      <code>{sourceLabel(workOrdersById.get(approval.work_order_id))}</code>
                      <ApprovalActions approvalId={approval.id} status={approval.status} />
                    </div>
                  ))
                )}
              </div>
            </div>

            <div className="panel">
              <div className="panelHeader">
                <h2>Needs review</h2>
                <span>{household.open_tasks.length} tasks</span>
              </div>
              <div className="taskList">
                {household.open_tasks.length === 0 ? (
                  <p className="emptyState">Review tasks will appear when Luna needs confirmation.</p>
                ) : (
                  household.open_tasks.map((task) => (
                    <div key={task.id} className="taskRow">
                      <strong>{task.title}</strong>
                      {task.description ? <span>{task.description}</span> : null}
                    </div>
                  ))
                )}
              </div>
            </div>

            <div className="panel">
              <div className="panelHeader">
                <h2>Prepared by Luna</h2>
                <span>{preparedWork.length} items</span>
              </div>
              <div className="taskList">
                {preparedWork.length === 0 ? (
                  <p className="emptyState">Work Luna prepares will appear here before it is completed.</p>
                ) : (
                  preparedWork.slice(0, 6).map((workOrder) => (
                    <div key={workOrder.id} className="taskRow">
                      <strong>{workOrder.title}</strong>
                      <span>{normalizedLabel(workOrder.work_type)} · {normalizedLabel(workOrder.status)}</span>
                      <code>{sourceLabel(workOrder)}</code>
                    </div>
                  ))
                )}
              </div>
            </div>

            <div className="panel">
              <div className="panelHeader">
                <h2>Upcoming obligations</h2>
                <span>{household.upcoming_obligations.length} monitored</span>
              </div>
              <div className="taskList">
                {household.upcoming_obligations.length === 0 ? (
                  <p className="emptyState">Confirmed obligations with due dates will appear here.</p>
                ) : (
                  household.upcoming_obligations.map((obligation) => (
                    <div key={obligation.id} className="taskRow">
                      <strong>{obligation.title}</strong>
                      <span>{obligationAmount(obligation)}</span>
                      <span className={`reminderPill ${reminderTone(obligation.due_date ?? new Date().toISOString())}`}>
                        {obligationDueLabel(obligation)}
                      </span>
                    </div>
                  ))
                )}
              </div>
            </div>

            <div className="panel">
              <div className="panelHeader">
                <h2>Overdue obligations</h2>
                <span>{household.overdue_obligations.length} overdue</span>
              </div>
              <div className="taskList">
                {household.overdue_obligations.length === 0 ? (
                  <p className="emptyState">Overdue household obligations will appear here.</p>
                ) : (
                  household.overdue_obligations.map((obligation) => (
                    <div key={obligation.id} className="taskRow">
                      <strong>{obligation.title}</strong>
                      <span>{obligationAmount(obligation)}</span>
                      <span className="reminderPill overdueReminder">
                        {obligationDueLabel(obligation)}
                      </span>
                    </div>
                  ))
                )}
              </div>
            </div>

            <div className="panel">
              <div className="panelHeader">
                <h2>Needs-review obligations</h2>
                <span>{household.needs_review_obligations.length} need review</span>
              </div>
              <div className="taskList">
                {household.needs_review_obligations.length === 0 ? (
                  <p className="emptyState">Untrusted or incomplete obligations will appear here.</p>
                ) : (
                  household.needs_review_obligations.map((obligation) => (
                    <div key={obligation.id} className="taskRow">
                      <strong>{obligation.title}</strong>
                      <span>{obligationAmount(obligation)}</span>
                      <span className="reminderPill soonReminder">
                        {obligationDueLabel(obligation)}
                      </span>
                    </div>
                  ))
                )}
              </div>
            </div>

            <div className="panel">
              <div className="panelHeader">
                <h2>Recently completed work</h2>
                <span>{recentlyCompletedWork.length} items</span>
              </div>
              <div className="taskList">
                {recentlyCompletedWork.length === 0 ? (
                  <p className="emptyState">Completed approvals, filings, and obligation work will appear here.</p>
                ) : (
                  recentlyCompletedWork.slice(0, 6).map((workOrder) => (
                    <div key={workOrder.id} className="taskRow">
                      <strong>{workOrder.title}</strong>
                      <span>{normalizedLabel(workOrder.work_type)} · {normalizedLabel(workOrder.status)}</span>
                      {Object.keys(workOrder.result).length > 0 ? (
                        <code>{JSON.stringify(workOrder.result)}</code>
                      ) : null}
                    </div>
                  ))
                )}
              </div>
            </div>

            <div className="panel">
              <div className="panelHeader">
                <h2>Recent activity</h2>
                <span>{auditEvents.length} events</span>
              </div>
              <div className="taskList">
                {auditEvents.length === 0 ? (
                  <p className="emptyState">Luna activity and user decisions will appear here.</p>
                ) : (
                  auditEvents.slice(0, 6).map((event) => (
                    <div key={event.id} className="taskRow">
                      <strong>{auditLabel(event)}</strong>
                      <span>{new Date(event.created_at).toLocaleString()}</span>
                      <code>{auditDetail(event)}</code>
                    </div>
                  ))
                )}
              </div>
            </div>
          </section>

          </div>

        </>
      ) : null}

      {activeTab === "bills" ? (
        <>
          <section className="tableWrap">
            <div className="tableHeader">
              <h2>Monitored obligations</h2>
              <span>{monitoredObligations.length} visible</span>
            </div>
            <table>
              <thead>
                <tr>
                  <th>Obligation</th>
                  <th>Amount</th>
                  <th>Due date</th>
                  <th>Status</th>
                  <th>Source record</th>
                  <th>Source document</th>
                </tr>
              </thead>
              <tbody>
                {monitoredObligations.map((obligation) => {
                  const sourceBill = obligation.source_bill_id
                    ? billsById.get(obligation.source_bill_id)
                    : undefined;
                  const sourceDocumentId = String(obligation.evidence.document_id ?? "");
                  const sourceDocument = sourceDocumentId
                    ? documentsById.get(sourceDocumentId)
                    : undefined;
                  return (
                    <tr key={obligation.id}>
                      <td>
                        <strong>{obligation.title}</strong>
                        <small>{obligation.supplier ?? "Supplier pending"}</small>
                      </td>
                      <td>{obligationAmount(obligation)}</td>
                      <td>{obligation.due_date ?? "Needs review"}</td>
                      <td>
                        <span className={`status obligation-${obligation.status}`}>
                          {obligation.status.replaceAll("_", " ")}
                        </span>
                      </td>
                      <td>
                        {sourceBill ? (
                          <>
                            <strong>{sourceBill.supplier}</strong>
                            <small>{obligation.source_bill_id}</small>
                          </>
                        ) : (
                          <small>{obligation.source_bill_id ?? "No source bill"}</small>
                        )}
                      </td>
                      <td>
                        {sourceDocument ? (
                          <>
                            <strong>{sourceDocument.original_filename}</strong>
                            <small>{sourceDocument.id}</small>
                          </>
                        ) : (
                          <small>{sourceDocumentId || "No source document"}</small>
                        )}
                      </td>
                    </tr>
                  );
                })}
              </tbody>
            </table>
            {monitoredObligations.length === 0 ? (
              <p className="emptyState tableEmpty">
                Obligations will appear after Luna reviews and confirms bills or due-date records.
              </p>
            ) : null}
          </section>

          <section className="tableWrap">
            <div className="tableHeader">
              <h2>Source records</h2>
              <span>{bills.length} records</span>
            </div>
            <table>
              <thead>
                <tr>
                  <th>Supplier</th>
                  <th>Amount</th>
                  <th>Due</th>
                  <th>Obligation</th>
                  <th>Category</th>
                  <th>Classification</th>
                  <th>Supplier entity</th>
                  <th>Luna confidence</th>
                  <th>Status</th>
                  <th>Graph</th>
                  <th>Actions</th>
                </tr>
              </thead>
              <tbody>
                {bills.map((bill) => (
                  <tr key={bill.id}>
                    <td>{bill.supplier}</td>
                    <td>{bill.amount == null ? "Pending" : `$${bill.amount.toFixed(2)}`}</td>
                    <td>{bill.due_date ?? "Pending"}</td>
                    <td>
                      <span className={`status ${bill.status}`}>{bill.status}</span>
                      {bill.document_id ? <small>Source document {bill.document_id}</small> : null}
                    </td>
                    <td>{bill.category ?? "Unsorted"}</td>
                    <td>{bill.classification ?? "Unclassified"}</td>
                    <td>
                      {bill.supplier_entity_id ? (
                        <span className="linkedEntity">
                          {graphNodesById.get(bill.supplier_entity_id)?.display_name ??
                            "Linked supplier"}
                        </span>
                      ) : (
                        <span className="unlinkedEntity">Unassigned</span>
                      )}
                    </td>
                    <td>
                      <span className={`status review-${bill.review_status}`}>
                        {bill.review_status === "needs_review"
                          ? "Needs confirmation"
                          : bill.review_status === "confirmed"
                            ? "Confirmed"
                            : "Looks right"}
                      </span>
                      {bill.review_reasons.length > 0 ? (
                        <small>{bill.review_reasons[0]}</small>
                      ) : null}
                    </td>
                    <td>
                      <span className={`status ${bill.status}`}>{bill.status}</span>
                    </td>
                    <td>
                      {!bill.supplier_entity_id ? (
                        <GraphLinkActions
                          nodes={householdGraph.nodes}
                          relationships={householdGraph.relationships}
                          sourceId={bill.id}
                          sourceType="bill"
                        />
                      ) : (
                        <small>{normalizedLabel("supplier_entity_linked")}</small>
                      )}
                    </td>
                    <td>
                      <BillActions billId={bill.id} status={bill.status} />
                    </td>
                  </tr>
                ))}
              </tbody>
            </table>
          </section>
        </>
      ) : null}

      {activeTab === "cabinet" ? (
        <>
          <section className="metrics" aria-label="Cabinet summary">
            <div>
              <span>Documents</span>
              <strong>{documents.length}</strong>
            </div>
            <div>
              <span>Suggested</span>
              <strong>{cabinetSuggestions.length}</strong>
            </div>
            <div>
              <span>Confirmed</span>
              <strong>{confirmedDocuments.length}</strong>
            </div>
            <div>
              <span>Needs review</span>
              <strong>{cabinetNeedsReview.length}</strong>
            </div>
          </section>

          <section className="tableWrap">
            <div className="tableHeader">
              <h2>Household records</h2>
              <span>{documents.length} records</span>
            </div>
            <div className="tableToolbar">
              <div className="toolbarCopy">
                <strong>Ask Luna to file a document.</strong>
                <span>
                  Luna will upload the record, prepare a filing suggestion, and ask
                  for approval before the path is confirmed.
                </span>
              </div>
              <UploadDocumentForm />
            </div>
            <form className="searchForm" action="/" aria-label="Search cabinet">
              <input type="hidden" name="tab" value="cabinet" />
              <input
                aria-label="Search household cabinet"
                defaultValue={cabinetQuery}
                name="q"
                placeholder="Search cabinet"
                type="search"
              />
              <button type="submit">Search</button>
            </form>
            {cabinetQuery.trim().length >= 2 ? (
              <p className="searchMeta">
                {`Showing ${cabinetDocuments.length} result${
                  cabinetDocuments.length === 1 ? "" : "s"
                } for "${cabinetQuery}"`}
              </p>
            ) : null}
            <table>
              <thead>
                <tr>
                  <th>Document</th>
                  <th>Status</th>
                  <th>Filing path</th>
                  <th>Luna filing work</th>
                  <th>Graph links</th>
                  <th>Actions</th>
                </tr>
              </thead>
              <tbody>
                {cabinetDocuments.map((document) => {
                  const filingWork = workOrders.find(
                    (workOrder) =>
                      workOrder.work_type === "document.cabinet_plan" &&
                      workOrder.source_document_id === document.id,
                  );
                  const filingApproval = approvalRequests.find(
                    (approval) => approval.work_order_id === filingWork?.id,
                  );
                  return (
                    <tr key={document.id}>
                      <td>{document.original_filename}</td>
                      <td>
                        <span className={`status cabinet-${document.cabinet_status}`}>
                          {document.cabinet_status.replaceAll("_", " ")}
                        </span>
                      </td>
                      <td>{cabinetPath(document) ? <code>{cabinetPath(document)}</code> : "Pending"}</td>
                      <td>
                        {filingWork ? (
                          <>
                            <strong>Luna prepared a filing suggestion</strong>
                            <small>
                              {filingApproval
                                ? `Approval ${filingApproval.status}`
                                : normalizedLabel(filingWork.status)}
                            </small>
                            <code>{sourceLabel(filingWork)}</code>
                          </>
                        ) : (
                          <small>No filing suggestion prepared yet.</small>
                        )}
                      </td>
                      <td>
                        <GraphLinkActions
                          nodes={householdGraph.nodes}
                          relationships={householdGraph.relationships}
                          sourceId={document.id}
                          sourceType="document"
                        />
                      </td>
                      <td>
                        <CabinetActions
                          cabinetStatus={document.cabinet_status}
                          documentId={document.id}
                        />
                      </td>
                    </tr>
                  );
                })}
              </tbody>
            </table>
          </section>
        </>
      ) : null}

      {activeTab === "structure" ? (
        <StructureEditor
          graph={householdGraph}
          initialMode={navigatorInitialMode}
          suggestions={graphSuggestions}
        />
      ) : null}

      {activeTab === "approvals" ? (
        <section className="tableWrap">
          <div className="tableHeader">
            <h2>Approval requests</h2>
            <span>{approvalRequests.length} records</span>
          </div>
          <table>
            <thead>
              <tr>
                <th>Request</th>
                <th>Status</th>
                <th>Approver</th>
                <th>Evidence</th>
                <th>Source</th>
                <th>Work order</th>
                <th>Decision</th>
              </tr>
            </thead>
            <tbody>
              {approvalRequests.map((approval) => {
                const workOrder = workOrdersById.get(approval.work_order_id);
                return (
                  <tr key={approval.id}>
                    <td>
                      <strong>{approval.reason}</strong>
                      {approval.decision_reason ? <small>{approval.decision_reason}</small> : null}
                    </td>
                    <td>
                      <span className={`status review-${approval.status}`}>
                        {approval.status.replaceAll("_", " ")}
                      </span>
                    </td>
                    <td>{approval.requested_approver_role ?? "Any authorised member"}</td>
                    <td>
                      {workOrder ? (
                        <div className="evidenceList">
                          {evidenceEntries(workOrder.evidence).map(([key, value]) => (
                            <span key={key}>
                              <strong>{normalizedLabel(key)}</strong>
                              {shortValue(value)}
                            </span>
                          ))}
                        </div>
                      ) : (
                        <small>Work evidence unavailable</small>
                      )}
                    </td>
                    <td><code>{sourceLabel(workOrder)}</code></td>
                    <td><code>{approval.work_order_id}</code></td>
                    <td>
                      <ApprovalActions approvalId={approval.id} status={approval.status} />
                    </td>
                  </tr>
                );
              })}
            </tbody>
          </table>
          {approvalRequests.length === 0 ? (
            <p className="emptyState">Luna has no approval requests yet.</p>
          ) : null}
        </section>
      ) : null}

      {activeTab === "assistant" ? (
        <section className="assistantLayout" aria-label="Household assistant">
          <div className="panel widePanel">
            <div className="panelHeader">
              <h2>Ask Luna</h2>
              <span>onboarding + grounded records</span>
            </div>
            <form className="searchForm inlineSearchForm" action="/" aria-label="Ask Luna">
              <input type="hidden" name="tab" value="assistant" />
              <input
                aria-label="Ask Luna a household question"
                defaultValue={assistantQuestion}
                name="ask"
                placeholder="Say hello, ask what is due, or ask what needs approval"
                type="search"
              />
              <button type="submit">Ask</button>
            </form>

            {assistantAnswer ? (
              <div className="answerBlock">
                <strong>{assistantAnswer.answer}</strong>
                <span>{Math.round(assistantAnswer.confidence * 100)}% grounded confidence</span>
              </div>
            ) : (
              <div className="answerBlock">
                <strong>
                  I'm Luna. I help manage household records, obligations, approvals,
                  and admin work.
                </strong>
                <span>
                  To start, upload a bill or document and I'll prepare it for review.
                </span>
              </div>
            )}
          </div>

          <div className="panel">
            <div className="panelHeader">
              <h2>Sources</h2>
              <span>{assistantAnswer?.sources.length ?? 0} records</span>
            </div>
            <div className="taskList">
              {assistantAnswer && assistantAnswer.sources.length > 0 ? (
                assistantAnswer.sources.map((source) => (
                  <div key={`${source.source_type}-${source.source_id}`} className="taskRow">
                    <strong>{source.title}</strong>
                    <span>{source.source_type}</span>
                    {source.detail ? <code>{source.detail}</code> : null}
                  </div>
                ))
              ) : (
                <p className="emptyState">
                  When Luna answers from household evidence, the source records will appear here.
                </p>
              )}
            </div>
          </div>

          <div className="panel">
            <div className="panelHeader">
              <h2>Next actions</h2>
              <span>{assistantAnswer?.suggested_next_actions.length ?? 0}</span>
            </div>
            <div className="taskList">
              {assistantAnswer && assistantAnswer.suggested_next_actions.length > 0 ? (
                assistantAnswer.suggested_next_actions.map((action) => (
                  <div key={action} className="taskRow">
                    <strong>{action}</strong>
                  </div>
                ))
              ) : (
                <div className="taskRow">
                  <strong>Upload a bill or document.</strong>
                  <span>Luna will prepare work, ask for approval where required, and record activity.</span>
                </div>
              )}
            </div>
          </div>
        </section>
      ) : null}

      {activeTab === "audit" ? (
        <section className="tableWrap">
          <div className="tableHeader">
            <h2>Luna work history</h2>
            <span>{auditEvents.length} events</span>
          </div>
          <table>
            <thead>
              <tr>
                <th>Event</th>
                <th>When</th>
                <th>Record</th>
                <th>Details</th>
              </tr>
            </thead>
            <tbody>
              {auditEvents.map((event) => (
                <tr key={event.id}>
                  <td>
                    <strong>{auditLabel(event)}</strong>
                    <small>{auditKind(event)}</small>
                  </td>
                  <td>{new Date(event.created_at).toLocaleString()}</td>
                  <td><code>{auditRecordLabel(event)}</code></td>
                  <td>
                    <div className="evidenceList">
                      {evidenceEntries(event.metadata).map(([key, value]) => (
                        <span key={key}>
                          <strong>{normalizedLabel(key)}</strong>
                          {shortValue(value)}
                        </span>
                      ))}
                    </div>
                  </td>
                </tr>
              ))}
            </tbody>
          </table>
          {auditEvents.length === 0 ? (
            <p className="emptyState tableEmpty">
              Luna's activity will show prepared work, approval requests, user decisions,
              obligation changes, reminders, and document filing decisions.
            </p>
          ) : null}
        </section>
      ) : null}

      {activeTab === "settings" ? (
        <section className="placeholderPanel">
          <strong>Settings will stay local-first.</strong>
          <span>
            Future controls for household preferences, cabinet paths, and storage choices will
            live here without changing the Phase 2 graph foundation.
          </span>
        </section>
      ) : null}
      </section>
    </main>
  );
}
