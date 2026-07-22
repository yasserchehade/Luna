import { FormEvent, useEffect, useState } from "react";
import type {
  ConversationService,
  FilingRuleReorganizationPreview,
  FilingRuleSummary,
  FilingRuleUpdate,
  ManualMoveCandidate,
} from "./conversationService";

function createdLabel(createdAt: string) {
  const timestamp = Number(createdAt);
  return Number.isFinite(timestamp) && timestamp > 0
    ? new Date(timestamp * 1000).toLocaleString()
    : "Unknown date";
}

function scopeLabel(rule: FilingRuleSummary) {
  const context = [rule.property && `property ${rule.property}`, rule.account && `account ${rule.account}`]
    .filter(Boolean)
    .join(" and ");
  return `${rule.documentType} from ${rule.serviceProvider} for ${rule.addressee}${context ? ` (${context})` : ""}`;
}

export function FilingRulesOptions({
  conversationService,
  householdId,
}: {
  conversationService: ConversationService;
  householdId: string;
}) {
  const [rules, setRules] = useState<FilingRuleSummary[]>([]);
  const [moveCandidates, setMoveCandidates] = useState<ManualMoveCandidate[]>([]);
  const [editing, setEditing] = useState<FilingRuleSummary | null>(null);
  const [draft, setDraft] = useState<FilingRuleUpdate | null>(null);
  const [preview, setPreview] = useState<FilingRuleReorganizationPreview | null>(null);
  const [error, setError] = useState("");
  const [isSubmitting, setIsSubmitting] = useState(false);

  const refresh = () => conversationService.listFilingRules(householdId).then(setRules);
  const refreshMoves = () => conversationService.listManualMoveCandidates(householdId).then(setMoveCandidates);

  useEffect(() => {
    void Promise.all([refresh(), refreshMoves()]).catch(() => setError("Luna could not load learned Filing Rules."));
  }, [conversationService, householdId]);

  const updateDraft = (field: keyof FilingRuleUpdate, value: string) => {
    setDraft((current) => current ? { ...current, [field]: value || null } : current);
  };

  const save = (event: FormEvent<HTMLFormElement>) => {
    event.preventDefault();
    if (!editing || !draft || isSubmitting) return;
    setIsSubmitting(true);
    setError("");
    void conversationService.updateFilingRule(householdId, editing.id, {
      ...draft,
      property: draft.property || null,
      account: draft.account || null,
    })
      .then(() => {
        setEditing(null);
        setDraft(null);
        setPreview(null);
        return refresh();
      })
      .catch(() => setError("Luna could not update that Filing Rule."))
      .finally(() => setIsSubmitting(false));
  };

  const togglePaused = (rule: FilingRuleSummary) => {
    setError("");
    void conversationService.pauseFilingRule(householdId, rule.id, !rule.paused)
      .then(refresh)
      .catch(() => setError("Luna could not change that Filing Rule."));
  };

  const recordMove = (candidate: ManualMoveCandidate, teachesRule: boolean) => {
    setError("");
    void conversationService.recordManualMoveDecision(householdId, candidate.arrivalId, teachesRule)
      .then(() => Promise.all([refresh(), refreshMoves()]))
      .catch(() => setError("Luna could not record that Cabinet move."));
  };

  const showPreview = () => {
    if (!editing || !draft) return;
    const directory = draft.cabinetDestination.split("/").slice(0, -1).join("/");
    setError("");
    void conversationService.previewFilingRuleReorganization(householdId, editing.id, directory)
      .then(setPreview)
      .catch(() => setError("Luna could not prepare the historical Filing Rule preview."));
  };

  const deleteRule = (rule: FilingRuleSummary) => {
    if (!window.confirm(`Delete the rule for ${scopeLabel(rule)}? Historical Originals will remain unchanged.`)) return;
    setError("");
    void conversationService.deleteFilingRule(householdId, rule.id)
      .then(refresh)
      .catch(() => setError("Luna could not delete that Filing Rule."));
  };

  return <section className="filing-rules" aria-label="Learned Filing Rules">
    <div className="section-heading"><div><small>Household learning</small><h2>Learned Filing Rules</h2></div><span>{rules.filter(({ deleted }) => !deleted).length} active</span></div>
    <p>These visible rules are the narrow permissions Luna earned from your Filing Decisions. Changes apply to future Document Arrivals; historical Originals stay where they are.</p>
    {error && <p className="account-error" role="alert">{error}</p>}
    {moveCandidates.length > 0 && <section className="manual-move-prompts" aria-label="Manual Cabinet moves">
      <h3>Owner Cabinet moves</h3>
      <p>Luna found managed Originals whose location changed. Choose whether each move teaches the matching Filing Rule or is a one-off exception.</p>
      {moveCandidates.map((candidate) => <article key={candidate.arrivalId}><strong>{candidate.originalName}</strong><small>{candidate.previousDestination} → {candidate.currentDestination}</small><div className="filing-rule-actions"><button type="button" onClick={() => recordMove(candidate, true)}>Teach this move</button><button type="button" onClick={() => recordMove(candidate, false)}>Keep as one-off</button></div></article>)}
    </section>}
    {rules.length === 0
      ? <p className="empty-state">No Filing Rules have been learned yet.</p>
      : <div className="filing-rule-list">{rules.map((rule) => <article className={`filing-rule-card${rule.deleted ? " deleted" : ""}`} data-rule-id={rule.id} key={rule.id}>
        <div className="filing-rule-card-heading"><div><small>{rule.deleted ? "Deleted rule" : rule.paused ? "Paused rule" : "Active rule"}</small><h3>{scopeLabel(rule)}</h3></div><span>{rule.affectedDocuments.length} affected</span></div>
        <p>File at <strong>{rule.cabinetDestination}</strong></p>
        <small>Learned from {rule.teacher} · Created {createdLabel(rule.createdAt)}</small>
        {rule.affectedDocuments.length > 0 && <details><summary>Affected Originals</summary><ul>{rule.affectedDocuments.map((document) => <li key={document}>{document}</li>)}</ul></details>}
        {!rule.deleted && <div className="filing-rule-actions">
          <button type="button" onClick={() => { setEditing(rule); setPreview(null); setDraft({
            documentType: rule.documentType,
            serviceProvider: rule.serviceProvider,
            addressee: rule.addressee,
            property: rule.property,
            account: rule.account,
            fileName: rule.fileName,
            cabinetDestination: rule.cabinetDestination,
          }); setError(""); }}>Edit rule</button>
          <button type="button" onClick={() => togglePaused(rule)}>{rule.paused ? "Resume rule" : "Pause rule"}</button>
          <button type="button" onClick={() => deleteRule(rule)}>Delete rule</button>
        </div>}
      </article>)}</div>}
    {editing && draft && <section className="filing-rule-editor" aria-label="Edit Filing Rule">
      <h3>Edit Filing Rule</h3>
      <p>Rule edits are prospective. Luna will show an exact historical preview before any future reorganisation is considered.</p>
      <form onSubmit={save}>
        <label>Document type<input aria-label="Rule document type" value={draft.documentType} onChange={(event) => updateDraft("documentType", event.target.value)} required /></label>
        <label>Service Provider<input aria-label="Rule Service Provider" value={draft.serviceProvider} onChange={(event) => updateDraft("serviceProvider", event.target.value)} required /></label>
        <label>Addressee<input aria-label="Rule Addressee" value={draft.addressee} onChange={(event) => updateDraft("addressee", event.target.value)} required /></label>
        <label>Property<input aria-label="Rule Property" value={draft.property ?? ""} onChange={(event) => updateDraft("property", event.target.value)} /></label>
        <label>Account<input aria-label="Rule Account" value={draft.account ?? ""} onChange={(event) => updateDraft("account", event.target.value)} /></label>
        <label>Filename<input aria-label="Rule filename" value={draft.fileName} onChange={(event) => updateDraft("fileName", event.target.value)} required /></label>
        <label className="wide-field">Cabinet Destination<input aria-label="Rule Cabinet Destination" value={draft.cabinetDestination} onChange={(event) => updateDraft("cabinetDestination", event.target.value)} required /></label>
        <div className="filing-rule-actions"><button type="submit" disabled={isSubmitting}>Save rule</button><button type="button" disabled={isSubmitting} onClick={showPreview}>Preview historical impact</button><button type="button" disabled={isSubmitting} onClick={() => { setEditing(null); setDraft(null); setPreview(null); }}>Cancel</button></div>
      </form>
      {preview && <section className="filing-rule-preview" aria-label="Historical Filing Rule preview">
        <strong>Exact historical preview for {preview.documents.length} Original{preview.documents.length === 1 ? "" : "s"}</strong>
        <p>This preview is read-only. Historical Originals stay unchanged until you explicitly approve a future reorganisation.</p>
        {preview.documents.length > 0 && <ul>{preview.documents.map((document) => <li key={document.arrivalId}><span>{document.originalName}</span><small>{document.currentDestination} → {document.proposedDestination}</small></li>)}</ul>}
      </section>}
    </section>}
  </section>;
}
