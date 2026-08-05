"use client";

import { useRef, type ChangeEvent, type FormEvent, type KeyboardEvent } from "react";
import { AppIcon } from "../../../components/AppIcon";
import type { AttachmentResult, HouseholdWorkView } from "../contracts";

export function PersistentComposer({
  work,
  draft,
  attachment,
  sending,
  attachmentPending,
  onDraftChange,
  onClearWork,
  onAttach,
  onClearAttachment,
  onSend,
}: {
  work: HouseholdWorkView | null;
  draft: string;
  attachment: AttachmentResult | null;
  sending: boolean;
  attachmentPending: boolean;
  onDraftChange: (value: string) => void;
  onClearWork: () => void;
  onAttach: (file: File) => void;
  onClearAttachment: () => void;
  onSend: () => void;
}) {
  const fileInput = useRef<HTMLInputElement>(null);
  const submit = (event: FormEvent) => { event.preventDefault(); onSend(); };
  const chooseFile = (event: ChangeEvent<HTMLInputElement>) => {
    const file = event.target.files?.[0];
    if (file) onAttach(file);
    event.target.value = "";
  };
  const submitFromKeyboard = (event: KeyboardEvent<HTMLTextAreaElement>) => {
    if (event.key === "Enter" && !event.shiftKey) {
      event.preventDefault();
      onSend();
    }
  };

  return (
    <form className="today-composer" aria-label="Delegate to Luna" onSubmit={submit}>
      {(work || attachment) && <div className="today-composer-context" aria-label="Active context">
        {work && <span><AppIcon name="spark" />{work.title}<button type="button" aria-label="Clear active work context" onClick={onClearWork}><AppIcon name="close" /></button></span>}
        {work && <span><AppIcon name="home" />{work.householdEntity}</span>}
        {work?.status === "awaitingApproval" && <span>Awaiting approval</span>}
        {attachment && <span><AppIcon name="paperclip" />{attachment.displayName} · {attachment.sizeLabel}<button type="button" aria-label={`Remove ${attachment.displayName}`} onClick={onClearAttachment}><AppIcon name="close" /></button></span>}
      </div>}
      <div className="today-composer-row">
        <input ref={fileInput} className="visually-hidden" type="file" accept=".pdf,.png,.jpg,.jpeg" aria-label="Attach a household document" onChange={chooseFile} />
        <button type="button" className="icon-button" aria-label="Attach a household document" disabled={attachmentPending} onClick={() => fileInput.current?.click()}><AppIcon name="paperclip" /></button>
        <textarea
          rows={1}
          aria-label="Instruction for Luna"
          value={draft}
          onChange={(event) => onDraftChange(event.target.value)}
          onKeyDown={submitFromKeyboard}
          placeholder={work ? `Reply about ${work.title.toLowerCase()}…` : "What would you like me to take care of?"}
        />
        <button type="submit" className="send-button" aria-label="Send instruction" disabled={sending || (!draft.trim() && !attachment)}><AppIcon name="send" /></button>
      </div>
      <small>PDF, JPG or PNG · 5 MB maximum</small>
    </form>
  );
}
