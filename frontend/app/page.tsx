import { UploadBillForm } from "../components/UploadBillForm";

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

type ActiveTab = "dashboard" | "cabinet" | "structure";

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

function normalizeTab(tab?: string | string[]): ActiveTab {
  const value = Array.isArray(tab) ? tab[0] : tab;
  if (value === "cabinet" || value === "structure") {
    return value;
  }
  return "dashboard";
}

function cabinetPath(document: DocumentRecord) {
  return document.confirmed_cabinet_path ?? document.suggested_cabinet_path;
}

export default async function DashboardPage({
  searchParams,
}: {
  searchParams?: Promise<{ tab?: string | string[]; q?: string | string[] }>;
}) {
  const resolvedSearchParams = await searchParams;
  const activeTab = normalizeTab(resolvedSearchParams?.tab);
  const cabinetQuery = Array.isArray(resolvedSearchParams?.q)
    ? resolvedSearchParams?.q[0] ?? ""
    : resolvedSearchParams?.q ?? "";
  const [bills, household, documents] = await Promise.all([
    getBills(),
    getHouseholdSummary(),
    getDocuments(),
  ]);
  const searchResults =
    activeTab === "cabinet" && cabinetQuery.trim().length >= 2
      ? await searchDocuments(cabinetQuery)
      : [];
  const cabinetDocuments =
    searchResults.length > 0
      ? searchResults.map((result) => result.document)
      : documents;

  const unpaid = bills.filter((bill) => bill.status === "unpaid");
  const overdue = bills.filter((bill) => bill.status === "overdue");
  const paid = bills.filter((bill) => bill.status === "paid");
  const needsReview = bills.filter((bill) => bill.review_status === "needs_review");
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
  const assetEntities = household.entities.filter((entity) =>
    ["asset", "property", "vehicle", "business", "family_trust"].includes(
      entity.entity_type,
    ),
  );

  return (
    <main className="shell">
      <section className="header">
        <div>
          <p className="eyebrow">Project Luna</p>
          <h1>Household operating system</h1>
        </div>
        <UploadBillForm />
      </section>

      <section className="tabs" aria-label="Luna sections">
        <a className={activeTab === "dashboard" ? "activeTab" : ""} href="/">
          Dashboard
        </a>
        <a className={activeTab === "cabinet" ? "activeTab" : ""} href="/?tab=cabinet">
          Cabinet
        </a>
        <a
          className={activeTab === "structure" ? "activeTab" : ""}
          href="/?tab=structure"
        >
          Structure
        </a>
      </section>

      {activeTab === "dashboard" ? (
        <>
          <section className="metrics" aria-label="Bill status summary">
            <div>
              <span>Unpaid</span>
              <strong>{unpaid.length}</strong>
            </div>
            <div>
              <span>Overdue</span>
              <strong>{overdue.length}</strong>
            </div>
            <div>
              <span>Needs review</span>
              <strong>{needsReview.length}</strong>
            </div>
            <div>
              <span>Paid</span>
              <strong>{paid.length}</strong>
            </div>
          </section>

          <section className="operationalGrid" aria-label="Household intelligence summary">
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
                      <span>{new Date(reminder.remind_at).toLocaleDateString()}</span>
                    </div>
                  ))
                )}
              </div>
            </div>
          </section>

          <section className="tableWrap">
            <div className="tableHeader">
              <h2>Bills and invoices</h2>
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
                  <th>Review</th>
                  <th>Status</th>
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
                      <span className={`status review-${bill.review_status}`}>
                        {bill.review_status.replaceAll("_", " ")}
                      </span>
                      {bill.review_reasons.length > 0 ? (
                        <small>{bill.review_reasons[0]}</small>
                      ) : null}
                    </td>
                    <td>
                      <span className={`status ${bill.status}`}>{bill.status}</span>
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
              <h2>Household cabinet</h2>
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
                  </tr>
                ))}
              </tbody>
            </table>
          </section>
        </>
      ) : null}

      {activeTab === "structure" ? (
        <section className="structureGrid" aria-label="Household structure">
          <div className="panel widePanel">
            <div className="panelHeader">
              <h2>Household structure</h2>
              <span>{household.entities.length} entities</span>
            </div>
            <div className="entityList">
              {household.entities.length === 0 ? (
                <p className="emptyState">Entities will appear as Luna reads household documents.</p>
              ) : (
                household.entities.map((entity) => (
                  <div key={entity.id} className="entityRow">
                    <strong>{entity.display_name}</strong>
                    <span>{entity.entity_type.replaceAll("_", " ")}</span>
                  </div>
                ))
              )}
            </div>
          </div>

          <div className="panel">
            <div className="panelHeader">
              <h2>Assets</h2>
              <span>{assetEntities.length} nodes</span>
            </div>
            <div className="entityList">
              {assetEntities.length === 0 ? (
                <p className="emptyState">Assets will appear as the household structure grows.</p>
              ) : (
                assetEntities.map((entity) => (
                  <div key={entity.id} className="entityRow">
                    <strong>{entity.display_name}</strong>
                    <span>{entity.entity_type.replaceAll("_", " ")}</span>
                  </div>
                ))
              )}
            </div>
          </div>
        </section>
      ) : null}
    </main>
  );
}
