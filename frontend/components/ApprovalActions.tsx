"use client";

import { useRouter } from "next/navigation";
import { useState } from "react";

type ApprovalActionsProps = {
  approvalId: string;
  status: string;
};

export function ApprovalActions({ approvalId, status }: ApprovalActionsProps) {
  const router = useRouter();
  const [loadingAction, setLoadingAction] = useState<string | null>(null);

  async function decide(action: "approve" | "reject" | "dismiss") {
    setLoadingAction(action);
    const baseUrl = process.env.NEXT_PUBLIC_API_BASE_URL ?? "http://localhost:8000";
    try {
      const response = await fetch(`${baseUrl}/api/work/approvals/${approvalId}/${action}`, {
        body: JSON.stringify({ reason: `User chose to ${action} this Luna work request.` }),
        headers: { "Content-Type": "application/json" },
        method: "POST",
      });
      if (!response.ok) {
        throw new Error("Approval decision failed.");
      }
      router.refresh();
    } finally {
      setLoadingAction(null);
    }
  }

  if (status !== "pending") {
    return <span className="mutedActionText">Decision recorded</span>;
  }

  return (
    <div className="inlineActionGroup">
      <button disabled={loadingAction !== null} onClick={() => decide("approve")} type="button">
        {loadingAction === "approve" ? "Approving" : "Approve"}
      </button>
      <button disabled={loadingAction !== null} onClick={() => decide("reject")} type="button">
        {loadingAction === "reject" ? "Rejecting" : "Reject"}
      </button>
      <button disabled={loadingAction !== null} onClick={() => decide("dismiss")} type="button">
        {loadingAction === "dismiss" ? "Dismissing" : "Dismiss"}
      </button>
    </div>
  );
}
