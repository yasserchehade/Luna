"use client";

import { useRouter } from "next/navigation";
import { useState } from "react";

type CabinetStatus = "unplanned" | "suggested" | "confirmed" | "filed" | "needs_review";
type ActionState = "idle" | "working" | "error";

type CabinetActionsProps = {
  cabinetStatus: CabinetStatus;
  documentId: string;
};

export function CabinetActions({ cabinetStatus, documentId }: CabinetActionsProps) {
  const router = useRouter();
  const [state, setState] = useState<ActionState>("idle");

  async function post(path: string, body?: object) {
    setState("working");
    const baseUrl = process.env.NEXT_PUBLIC_API_BASE_URL ?? "http://localhost:8000";

    try {
      const response = await fetch(`${baseUrl}/api/documents/${documentId}/${path}`, {
        body: body ? JSON.stringify(body) : undefined,
        headers: body ? { "Content-Type": "application/json" } : undefined,
        method: "POST",
      });
      if (!response.ok) {
        throw new Error("Cabinet action failed.");
      }
      setState("idle");
      router.refresh();
    } catch {
      setState("error");
    }
  }

  return (
    <div className="rowActions">
      {cabinetStatus === "unplanned" || cabinetStatus === "needs_review" ? (
        <button disabled={state === "working"} onClick={() => post("cabinet-plan")} type="button">
          Plan
        </button>
      ) : null}
      {cabinetStatus === "suggested" ? (
        <button
          disabled={state === "working"}
          onClick={() => post("cabinet-confirm", { cabinet_path: null })}
          type="button"
        >
          Confirm
        </button>
      ) : null}
      {state === "error" ? <span role="alert">Failed</span> : null}
    </div>
  );
}
