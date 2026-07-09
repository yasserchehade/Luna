import { BillActions } from "../components/BillActions";
import { CabinetActions } from "../components/CabinetActions";
import { CreateMenu } from "../components/CreateMenu";
import { GraphLinkActions } from "../components/GraphLinkActions";
import { StructureEditor } from "../components/StructureEditor";

type Bill = {
  id: string;
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

type HouseholdSummary = {
  entities: HouseholdEntity[];
  open_tasks: HouseholdTask[];
  upcoming_reminders: HouseholdReminder[];
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
  { href: "/", icon: "WB", label: "Workbench", tab: "dashboard" },
  { href: "/?tab=cabinet", icon: "RC", label: "Records", tab: "cabinet" },
  { href: "/?tab=bills", icon: "OB", label: "Obligations", tab: "bills" },
  { href: "/?tab=structure", icon: "HH", label: "Household", tab: "structure" },
  { href: "/?tab=approvals", icon: "AP", label: "Approvals", tab: "approvals" },
  { href: "/?tab=assistant", icon: "LN", label: "Luna", tab: "assistant" },
  { href: "/?tab=audit", icon: "AU", label: "Audit Log", tab: "audit" },
  { href: "/?tab=settings", icon: "SE", label: "Settings", tab: "settings" },
];

const PAGE_META: Record<ActiveTab, { title: string; subtitle: string }> = {
  dashboard: {
    title: "Luna workbench",
    subtitle: "Prepared work, approvals, obligations, and reminders needing household attention.",
  },
  cabinet: {
    title: "Household records",
    subtitle: "A searchable cabinet of source documents Luna organizes and understands.",
  },
  bills: {
    title: "Household obligations",
    subtitle: "Bills, invoices, due dates, and payable work Luna has prepared.",
  },
  structure: {
    title: "Household map",
    subtitle: "Members, properties, vehicles, suppliers, policies, accounts, and relationships.",
  },
  approvals: {
    title: "Approvals",
    subtitle: "Work Luna has prepared and is waiting for an authorised decision.",
  },
  assistant: {
    title: "Luna",
    subtitle: "Ask Luna questions grounded in your household records and work queue.",
  },
  audit: {
    title: "Audit log",
    subtitle: "Important household changes, approval decisions, and Luna work history.",
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
      return { entities: [], open_tasks: [], upcoming_reminders: [] };
    }
    return response.json();
  } catch {
    return { entities: [], open_tasks: [], upcoming_reminders: [] };
  }
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
  return value.replaceAll("_", " ");
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

export default async function DashboardPage({
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
  const [bills, household, householdGraph, documents, graphSuggestions, approvalRequests] = await Promise.all([
    getBills(),
    getHouseholdSummary(),
    getHouseholdGraph(),
    getDocuments(),
    getGraphSuggestions(),
    getApprovalRequests(),
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

  const unpaid = bills.filter((bill) => bill.status === "unpaid");
  const overdue = bills.filter((bill) => bill.status === "overdue");
  const paid = bills.filter((bill) => bill.status === "paid");
  const needsReview = bills.filter((bill) => bill.review_status === "needs_review");
  const pendingApprovals = approvalRequests.filter((approval) => approval.status === "pending");
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
  const pageMeta = PAGE_META[activeTab];
  return (
    <main className="appShell">
      <aside className="sidebar" aria-label="Luna navigation">
        <div className="brandBlock">
          <span className="brandMark">L</span>
          <div>
            <strong>Luna</strong>
            <span>AI household employee</span>
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
            <strong>Demo household</strong>
            <small>Local workspace</small>
          </div>
        </div>
      </aside>

      <section className="mainContent">
        <section className="pageHeader">
          <div>
            <p className="eyebrow">Project Luna</p>
            <h1>{pageMeta.title}</h1>
            <span>{pageMeta.subtitle}</span>
          </div>
          <div className="pageHeaderActions">
            <a className="assistantButton" href="/?tab=assistant">
              Ask Luna
            </a>
            <CreateMenu />
          </div>
        </section>

      {activeTab === "dashboard" ? (
        <>
          <section className="metrics" aria-label="Luna workbench summary">
            <div>
              <span>Unpaid</span>
              <strong>{unpaid.length}</strong>
            </div>
            <div>
              <span>Overdue</span>
              <strong>{overdue.length}</strong>
            </div>
            <div>
              <span>Needs approval</span>
              <strong>{pendingApprovals.length}</strong>
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
                    </div>
                  ))
                )}
              </div>
            </div>

            <div className="panel">
              <div className="panelHeader">
                <h2>Needs attention</h2>
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
                <h2>Upcoming reminders</h2>
                <span>{household.upcoming_reminders.length} scheduled</span>
              </div>
              <div className="taskList">
                {household.upcoming_reminders.length === 0 ? (
                  <p className="emptyState">Due-date reminders will appear after extraction.</p>
                ) : (
                  household.upcoming_reminders.map((reminder) => (
                    <div key={reminder.id} className="taskRow">
                      <strong>{reminder.title}</strong>
                      <span className={`reminderPill ${reminderTone(reminder.remind_at)}`}>
                        {reminderTiming(reminder.remind_at)}
                      </span>
                    </div>
                  ))
                )}
              </div>
            </div>
          </section>

        </>
      ) : null}

      {activeTab === "bills" ? (
        <>
          <section className="tableWrap">
            <div className="tableHeader">
              <h2>Household obligations</h2>
              <span>{bills.length} records</span>
            </div>
            <table>
              <thead>
                <tr>
                  <th>Supplier</th>
                  <th>Amount</th>
                  <th>Due</th>
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
                  <th>Cabinet path</th>
                  <th>Graph links</th>
                  <th>Actions</th>
                </tr>
              </thead>
              <tbody>
                {cabinetDocuments.map((document) => (
                  <tr key={document.id}>
                    <td>{document.original_filename}</td>
                    <td>
                      <span className={`status cabinet-${document.cabinet_status}`}>
                        {document.cabinet_status.replaceAll("_", " ")}
                      </span>
                    </td>
                    <td>{cabinetPath(document) ? <code>{cabinetPath(document)}</code> : "Pending"}</td>
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
                ))}
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
                <th>Work order</th>
              </tr>
            </thead>
            <tbody>
              {approvalRequests.map((approval) => (
                <tr key={approval.id}>
                  <td>{approval.reason}</td>
                  <td>
                    <span className={`status review-${approval.status}`}>
                      {approval.status.replaceAll("_", " ")}
                    </span>
                  </td>
                  <td>{approval.requested_approver_role ?? "Any authorised member"}</td>
                  <td><code>{approval.work_order_id}</code></td>
                </tr>
              ))}
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
              <span>grounded beta</span>
            </div>
            <form className="searchForm inlineSearchForm" action="/" aria-label="Ask Luna">
              <input type="hidden" name="tab" value="assistant" />
              <input
                aria-label="Ask Luna a household question"
                defaultValue={assistantQuestion}
                name="ask"
                placeholder="Ask what is due, what needs review, or find a document"
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
              <p className="emptyState">Ask a question to search Luna's structured household memory.</p>
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
                <p className="emptyState">Luna will show the records behind each answer here.</p>
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
                <p className="emptyState">Suggested actions will appear when Luna has a useful next step.</p>
              )}
            </div>
          </div>
        </section>
      ) : null}

      {activeTab === "audit" ? (
        <section className="placeholderPanel">
          <strong>Audit log is coming into focus.</strong>
          <span>
            Luna already records key household events, work orders, and approval decisions
            in the backend. This view will become the household work history.
          </span>
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
