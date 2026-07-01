"use client";

import { useRouter } from "next/navigation";
import { FormEvent, useRef, useState } from "react";

type UploadState = "idle" | "uploading" | "error";

export function UploadBillForm() {
  const router = useRouter();
  const inputRef = useRef<HTMLInputElement>(null);
  const [state, setState] = useState<UploadState>("idle");

  async function handleSubmit(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    const file = inputRef.current?.files?.[0];
    if (!file) {
      return;
    }

    setState("uploading");
    const baseUrl = process.env.NEXT_PUBLIC_API_BASE_URL ?? "http://localhost:8000";
    const formData = new FormData();
    formData.append("file", file);

    try {
      const documentResponse = await fetch(`${baseUrl}/api/documents`, {
        method: "POST",
        body: formData,
      });
      if (!documentResponse.ok) {
        throw new Error("Upload failed.");
      }

      const document = (await documentResponse.json()) as { id: string };
      const ingestResponse = await fetch(`${baseUrl}/api/bills/ingest`, {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify({ document_id: document.id }),
      });
      if (!ingestResponse.ok) {
        throw new Error("Ingest failed.");
      }

      if (inputRef.current) {
        inputRef.current.value = "";
      }
      setState("idle");
      router.refresh();
    } catch {
      setState("error");
    }
  }

  return (
    <form className="uploadForm" onSubmit={handleSubmit}>
      <label className="filePicker">
        <span>PDF bill</span>
        <input ref={inputRef} type="file" name="file" accept="application/pdf" />
      </label>
      <button type="submit" disabled={state === "uploading"}>
        {state === "uploading" ? "Uploading" : "Upload"}
      </button>
      {state === "error" ? <p role="alert">Upload failed</p> : null}
    </form>
  );
}
