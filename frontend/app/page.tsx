import { UploadBillForm } from "../components/UploadBillForm";

type Bill = {
  id: string;
  supplier: string;
  amount?: number | null;
  due_date?: string | null;
  invoice_number?: string;
  category?: string;
  classification?: string;
  status: "draft" | "unpaid" | "paid" | "overdue" | "archived";
};

async function getBills(): Promise<Bill[]> {
  const baseUrl =
    process.env.API_INTERNAL_BASE_URL ??
    process.env.NEXT_PUBLIC_API_BASE_URL ??
    "http://localhost:8000";

  try {
    const response = await fetch(`${baseUrl}/api/bills`, { cache: "no-store" });
    if (!response.ok) {
      return [];
    }
    return response.json();
  } catch {
    return [];
  }
}

export default async function DashboardPage() {
  const bills = await getBills();
  const unpaid = bills.filter((bill) => bill.status === "unpaid");
  const overdue = bills.filter((bill) => bill.status === "overdue");
  const paid = bills.filter((bill) => bill.status === "paid");
  const upcoming = unpaid.slice(0, 5);

  return (
    <main className="shell">
      <section className="header">
        <div>
          <p className="eyebrow">Project Luna</p>
          <h1>Administrative operating system</h1>
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
          <span>Upcoming</span>
          <strong>{upcoming.length}</strong>
        </div>
        <div>
          <span>Paid</span>
          <strong>{paid.length}</strong>
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
