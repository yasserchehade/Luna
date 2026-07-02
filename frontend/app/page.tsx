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

export default async function DashboardPage() {
  const [bills, household] = await Promise.all([getBills(), getHouseholdSummary()]);
  const unpaid = bills.filter((bill) => bill.status === "unpaid");
  const overdue = bills.filter((bill) => bill.status === "overdue");
  const paid = bills.filter((bill) => bill.status === "paid");
  const needsReview = bills.filter((bill) => bill.review_status === "needs_review");

  return (
    <main className="shell">
      <section className="header">
        <div>
          <p className="eyebrow">Project Luna</p>
          <h1>Household operating system</h1>
        </div>
        <UploadBillForm />
      </section>

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

      <section className="householdGrid" aria-label="Household intelligence summary">
        <div className="panel">
          <div className="panelHeader">
            <h2>Household memory</h2>
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
    </main>
  );
}
