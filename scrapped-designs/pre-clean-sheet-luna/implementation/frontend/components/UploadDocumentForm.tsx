"use client";

import { useRouter } from "next/navigation";
import { FormEvent, useRef, useState } from "react";

type UploadState = "idle" | "uploading" | "error";

export function UploadDocumentForm() {
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
        body: formData,
        method: "POST",
      });
      if (!documentResponse.ok) {
        throw new Error("Document upload failed.");
      }

      const document = (await documentResponse.json()) as { id: string };
      const planResponse = await fetch(`${baseUrl}/api/documents/${document.id}/cabinet-plan`, {
        method: "POST",
      });
      if (!planResponse.ok) {
        throw new Error("Filing suggestion failed.");
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
    <form className="uploadForm compactUploadForm" onSubmit={handleSubmit}>
      <label className="filePicker">
        <span>PDF household record</span>
        <input ref={inputRef} type="file" name="file" accept="application/pdf" />
      </label>
      <button type="submit" disabled={state === "uploading"}>
        {state === "uploading" ? "Preparing" : "Ask Luna to file a document"}
      </button>
      {state === "error" ? <p role="alert">Luna could not prepare this filing suggestion</p> : null}
    </form>
  );
}
