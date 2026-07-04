"use client";

import { useRouter } from "next/navigation";
import { useState } from "react";

type BillStatus = "draft" | "unpaid" | "paid" | "overdue" | "archived";
type ActionState = "idle" | "working" | "error";

type BillActionsProps = {
  billId: string;
  status: BillStatus;
};

export function BillActions({ billId, status }: BillActionsProps) {
  const router = useRouter();
  const [state, setState] = useState<ActionState>("idle");

  async function runAction(path: string) {
    setState("working");
    const baseUrl = process.env.NEXT_PUBLIC_API_BASE_URL ?? "http://localhost:8000";

    try {
      const response = await fetch(`${baseUrl}/api/bills/${billId}/${path}`, {
        method: "POST",
      });
      if (!response.ok) {
        throw new Error("Bill action failed.");
      }
      setState("idle");
      router.refresh();
    } catch {
      setState("error");
    }
  }

  return (
    <div className="rowActions">
      {status === "draft" ? (
        <button disabled={state === "working"} onClick={() => runAction("confirm")} type="button">
          Confirm
        </button>
      ) : null}
      {status !== "paid" && status !== "archived" ? (
        <button disabled={state === "working"} onClick={() => runAction("mark-paid")} type="button">
          Paid
        </button>
      ) : null}
      {status !== "archived" ? (
        <button disabled={state === "working"} onClick={() => runAction("archive")} type="button">
          Archive
        </button>
      ) : null}
      {state === "error" ? <span role="alert">Failed</span> : null}
    </div>
  );
}
