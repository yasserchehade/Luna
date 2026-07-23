import { FormEvent, useCallback, useEffect, useRef, useState } from "react";
import { getCurrentWebview } from "@tauri-apps/api/webview";
import type {
  Conversation,
  ConversationMessage,
  ConversationService,
  CloudConsentDecision,
  CloudConsentScope,
  DocumentContextDirection,
  DocumentArrival,
  DuplicateDecision,
  FilingDecisionDirection,
  IntelligenceProviderStatus,
  TodoItem,
} from "./conversationService";

type ConversationWorkspaceProps = {
  conversationService: ConversationService;
  destination: "Luna" | "To do";
  householdId: string;
  cabinetRecoveryRequest: number;
  newConversationRequest: number;
  conversationSelectionRequest: { conversationId: number; request: number } | null;
  onRecentConversationsChange(conversations: Conversation[]): void;
  onActiveConversationChange(conversationId: number | null): void;
  householdName: string;
  onOpenConversation(): void;
  onTodoCountChange(count: number): void;
};

const stateLabel = (arrival: DocumentArrival) => ({
  needsMemberDirection: "Needs your direction",
  possibleDuplicate: "Needs duplicate decision",
  readyToFile: "Ready to file",
  filing: "Filing",
  waitingForConnectivity: "Waiting for provider",
  cabinetUnavailable: "Waiting for Cabinet",
  filed: "Filed",
  dismissed: "Dismissed",
})[arrival.processingState];

const confidenceLabel = (arrival: DocumentArrival) => ({
  confirmed: "Confirmed",
  looksRight: "Looks right",
  needsChecking: "Needs checking",
  unknown: "Unknown",
})[arrival.reviewCard.confidenceState];

const duplicateDecisionOptions: Array<[DuplicateDecision, string]> = [
  ["keepBoth", "Keep both"],
  ["linkCopies", "Link copies"],
  ["discardNew", "Discard new"],
  ["updatedVersion", "Updated version"],
];

const duplicateResolutionLabels: Record<DuplicateDecision, string> = {
  keepBoth: "Kept both Originals",
  linkCopies: "Linked both Originals",
  discardNew: "Discarded the new Original",
  updatedVersion: "Kept both Originals as an updated version",
};

type DocumentReviewEditorProps = {
  arrival: DocumentArrival;
  conversationService: ConversationService;
  householdId: string;
  onConfirm(direction: FilingDecisionDirection): Promise<void>;
  onRecord(direction: DocumentContextDirection): Promise<void>;
  onResolveDuplicate(relatedArrivalId: number, decision: DuplicateDecision, rememberPreference: boolean): Promise<void>;
  onRefresh(): Promise<void>;
};

const cloudContextFields = [
  "documentType",
  "serviceProvider",
  "serviceProviderRelevance",
  "addressee",
  "property",
  "propertyRelevance",
  "account",
  "amount",
  "relevantDates",
] as const;

const cloudEvidence = (arrival: DocumentArrival): Array<{ field: string; value: string }> => [
  { field: "documentName", value: arrival.originalName },
  { field: "mediaType", value: arrival.mediaType },
  ...(arrival.extractedText ? [{ field: "extractedText", value: arrival.extractedText }] : []),
  ...cloudContextFields.flatMap((field): Array<{ field: string; value: string }> => {
    const context = arrival.reviewCard.context;
    if (field === "relevantDates") {
      const value = context.relevantDates.flatMap(({ value }) => value ? [value] : []).join(", ");
      return value ? [{ field, value }] : [];
    }
    const value = context[field].value;
    return value ? [{ field, value }] : [];
  }),
];

const applyCloudFields = (
  current: DocumentContextDirection,
  fields: Record<string, string>,
): DocumentContextDirection => {
  const next = { ...current };
  const assign = (field: "documentType" | "serviceProvider" | "addressee" | "property" | "account" | "amount", value: string | undefined) => {
    const normalized = value?.trim();
    if (!normalized) return;
    next[field] = normalized;
    next[`${field}Resolved`] = true;
  };
  assign("documentType", fields.documentType);
  assign("serviceProvider", fields.serviceProvider);
  assign("addressee", fields.addressee);
  assign("property", fields.property);
  assign("account", fields.account);
  assign("amount", fields.amount);
  if (fields.serviceProviderRelevance?.trim()) {
    next.serviceProviderRelevance = {
      subject: next.serviceProvider ?? "",
      explanation: fields.serviceProviderRelevance.trim(),
    };
  }
  if (fields.propertyRelevance?.trim()) {
    next.propertyRelevance = {
      subject: next.property ?? "",
      explanation: fields.propertyRelevance.trim(),
    };
  }
  if (fields.relevantDates?.trim()) {
    next.relevantDates = fields.relevantDates.split(",").map((date) => date.trim()).filter(Boolean);
    next.relevantDatesResolved = next.relevantDates.length > 0;
  }
  return next;
};

const directionFromReview = (arrival: DocumentArrival): DocumentContextDirection => {
  const context = arrival.reviewCard.context;
  const isConfirmed = (confidenceState: string) => confidenceState === "confirmed";
  return {
    documentType: context.documentType.value,
    documentTypeResolved: isConfirmed(context.documentType.confidenceState),
    serviceProvider: context.serviceProvider.value,
    serviceProviderResolved: isConfirmed(context.serviceProvider.confidenceState),
    serviceProviderRelevance: context.serviceProviderRelevance.value
      ? {
        subject: context.serviceProvider.value ?? "",
        explanation: context.serviceProviderRelevance.value,
      }
      : null,
    addressee: context.addressee.value,
    addresseeResolved: isConfirmed(context.addressee.confidenceState),
    property: context.property.value,
    propertyResolved: isConfirmed(context.property.confidenceState),
    propertyRelevance: context.propertyRelevance.value
      ? {
        subject: context.property.value ?? "",
        explanation: context.propertyRelevance.value,
      }
      : null,
    account: context.account.value,
    accountResolved: isConfirmed(context.account.confidenceState),
    amount: context.amount.value,
    amountResolved: isConfirmed(context.amount.confidenceState),
    relevantDates: context.relevantDates.flatMap(({ value }) => value ? [value] : []),
    relevantDatesResolved: context.relevantDates.length > 0
      ? context.relevantDates.every(({ confidenceState }) => isConfirmed(confidenceState))
      : arrival.reviewCard.questions.every(({ field }) => field !== "relevantDates"),
  };
};

function DocumentReviewEditor({
  arrival,
  conversationService,
  householdId,
  onConfirm,
  onRecord,
  onResolveDuplicate,
  onRefresh,
}: DocumentReviewEditorProps) {
  const context = arrival.reviewCard.context;
  const [direction, setDirection] = useState<DocumentContextDirection>(
    () => directionFromReview(arrival),
  );
  const [datesDraft, setDatesDraft] = useState(direction.relevantDates.join(", "));
  const decision = arrival.reviewCard.filingDecision;
  const [fileName, setFileName] = useState(decision?.fileName ?? "");
  const [cabinetDestination, setCabinetDestination] = useState(
    decision?.cabinetDestination ?? "",
  );
  const [rememberDuplicatePreference, setRememberDuplicatePreference] = useState(false);
  const [cloudOpen, setCloudOpen] = useState(false);
  const [cloudProviders, setCloudProviders] = useState<IntelligenceProviderStatus[]>([]);
  const [cloudScopes, setCloudScopes] = useState<CloudConsentScope[]>([]);
  const [cloudProviderId, setCloudProviderId] = useState("");
  const [cloudBusy, setCloudBusy] = useState(false);
  const [cloudMessage, setCloudMessage] = useState("");
  const [cloudError, setCloudError] = useState("");
  const clarificationQuestions = arrival.processingState === "needsMemberDirection"
    ? arrival.reviewCard.questions.filter(({ field }) => field !== "amount")
    : arrival.reviewCard.questions;
  const unresolvedFields = arrival.reviewCard.questions.map(({ field }) => field);
  const cloudScopeFields = Array.from(new Set([
    ...unresolvedFields,
    ...cloudEvidence(arrival).map(({ field }) => field),
  ]));
  const selectedCloudProvider = cloudProviders.find(({ descriptor }) => descriptor.id === cloudProviderId);
  const existingCloudScope = cloudScopes.find((scope) => (
    !scope.revoked
    && scope.providerId === cloudProviderId
    && scope.purpose === "document-evaluation"
    && cloudScopeFields.every((field) => scope.fields.includes(field))
  ));

  useEffect(() => {
    const refreshed = directionFromReview(arrival);
    setDirection(refreshed);
    setDatesDraft(refreshed.relevantDates.join(", "));
  }, [arrival, context]);

  useEffect(() => {
    setFileName(decision?.fileName ?? "");
    setCabinetDestination(decision?.cabinetDestination ?? "");
  }, [decision]);

  useEffect(() => {
    setCloudOpen(false);
    setCloudProviders([]);
    setCloudScopes([]);
    setCloudProviderId("");
    setCloudMessage("");
    setCloudError("");
  }, [arrival.id]);

  const openCloudAssistance = async () => {
    setCloudOpen(true);
    setCloudBusy(true);
    setCloudError("");
    try {
      const [providers, scopes] = await Promise.all([
        conversationService.listIntelligenceProviderStatuses(householdId),
        conversationService.listCloudConsentScopes(householdId),
      ]);
      setCloudProviders(providers);
      setCloudScopes(scopes);
      setCloudProviderId((current) => current || providers.find(({ configured }) => configured)?.descriptor.id || providers[0]?.descriptor.id || "");
    } catch (reason) {
      setCloudError(String(reason));
    } finally {
      setCloudBusy(false);
    }
  };

  const askCloudProvider = async (consent: CloudConsentDecision) => {
    if (!cloudProviderId) return;
    setCloudBusy(true);
    setCloudError("");
    setCloudMessage("");
    try {
      const result = await conversationService.evaluateCloudRequest(
        householdId,
        {
          purpose: "document-evaluation",
          documentName: arrival.originalName,
          mediaType: arrival.mediaType,
          evidence: cloudEvidence(arrival),
          unresolvedFields,
        },
        cloudProviderId,
        consent,
      );
      if (arrival.processingState === "waitingForConnectivity") {
        await conversationService.resumeWaitingDocument(householdId, arrival.id);
        await onRefresh();
      }
      const suggestedFields = Object.keys(result.fields);
      if (suggestedFields.length === 0) {
        setCloudMessage(`${selectedCloudProvider?.descriptor.name ?? "The provider"} returned no usable suggestions. Luna kept this review local.`);
      } else {
        setDirection((current) => applyCloudFields(current, result.fields));
        setCloudMessage(`${selectedCloudProvider?.descriptor.name ?? "The provider"} suggested ${suggestedFields.join(", ")}. Review the fields, then save Household Context.`);
      }
      if (consent === "allowForScope") {
        const scopes = await conversationService.listCloudConsentScopes(householdId);
        setCloudScopes(scopes);
      }
    } catch (reason) {
      if (consent === "keepLocal") {
        if (arrival.processingState === "waitingForConnectivity") {
          await conversationService.resumeWaitingDocument(householdId, arrival.id);
          await onRefresh();
        }
        setCloudMessage("Kept local. No document data was sent to a provider.");
      } else {
        const reasonText = String(reason);
        if (reasonText.includes("provider is unavailable")) {
          await conversationService.markDocumentWaitingForConnectivity(householdId, arrival.id);
          await onRefresh();
          setCloudMessage("The provider is unavailable. Luna kept this review waiting and will retry when connectivity returns.");
        } else {
          setCloudError(reasonText);
        }
      }
    } finally {
      setCloudBusy(false);
    }
  };

  const setField = (
    field: "documentType" | "serviceProvider" | "addressee" | "property" | "account" | "amount",
    value: string,
  ) => setDirection((current) => {
    const next = { ...current, [field]: value || null };
    if (field === "serviceProvider" && value !== current.serviceProvider) {
      next.serviceProviderRelevance = null;
    }
    if (field === "property" && value !== current.property) {
      next.propertyRelevance = null;
    }
    return next;
  });

  return <section className="review-card" aria-label={`Review card for ${arrival.originalName}`}>
    <strong>{confidenceLabel(arrival)}</strong>
    <dl>{arrival.reviewCard.evidence.map((evidence) => <div key={evidence.label}>
      <dt>{evidence.label}</dt><dd>{evidence.value}</dd>
    </div>)}</dl>
    {arrival.duplicateReview && <section className="duplicate-review" aria-label={`Duplicate review for ${arrival.originalName}`}>
      <strong>{arrival.duplicateReview.candidates[0]?.kind === "exact" ? "Exact byte duplicate" : "Possible duplicate (changed bytes)"}</strong>
      <p>Luna found one or more Originals that may represent the same document. Choose what should happen to this new arrival in relation to each candidate.</p>
      <ul>{arrival.duplicateReview.candidates.map((candidate) => <li key={candidate.arrivalId}>
        <span>{candidate.originalName}</span>
        <small>{candidate.kind === "exact" ? "Exact SHA-256 match" : "Same Household context with changed bytes"}{candidate.filedDestination ? ` · Filed at ${candidate.filedDestination}` : ""}</small>
        {candidate.kind === "exact" && <label className="duplicate-preference"><input type="checkbox" checked={rememberDuplicatePreference} onChange={(event) => setRememberDuplicatePreference(event.target.checked)} /> Remember this choice for this exact duplicate scope</label>}
        <div className="duplicate-actions">{duplicateDecisionOptions.map(([decisionValue, label]) => <button
          key={decisionValue}
          type="button"
          onClick={() => void onResolveDuplicate(
            candidate.arrivalId,
            decisionValue,
            candidate.kind === "exact" && rememberDuplicatePreference,
          )}
        >{label}</button>)}</div>
      </li>)}</ul>
    </section>}
    {arrival.duplicateResolution && <aside className="duplicate-resolution" aria-label="Duplicate resolution">
      <small>Duplicate decision</small>
      <p>{duplicateResolutionLabels[arrival.duplicateResolution.decision]}</p>
      <small>Related Original: {arrival.duplicateResolution.relatedOriginalName}</small>
    </aside>}
    {arrival.processingState === "needsMemberDirection" && <form className="context-review-form" onSubmit={(event) => {
      event.preventDefault();
      void onRecord({
        ...direction,
        relevantDates: datesDraft.split(",").map((date) => date.trim()).filter(Boolean),
        documentTypeResolved: true,
        serviceProviderResolved: true,
        addresseeResolved: true,
        propertyResolved: true,
        accountResolved: true,
        amountResolved: true,
        relevantDatesResolved: true,
      });
    }}>
      <label>Document type<input aria-label="Document type" value={direction.documentType ?? ""} onChange={(event) => setField("documentType", event.target.value)} /></label>
      <label>Service Provider<input aria-label="Service Provider" value={direction.serviceProvider ?? ""} onChange={(event) => setField("serviceProvider", event.target.value)} /></label>
      <label>Service provider relevance<input aria-label="Service Provider relevance" value={direction.serviceProviderRelevance?.explanation ?? ""} onChange={(event) => setDirection((current) => ({
        ...current,
        serviceProviderRelevance: event.target.value ? {
          subject: current.serviceProvider ?? "",
          explanation: event.target.value,
        } : null,
      }))} /></label>
      <label>Addressee<input aria-label="Addressee" value={direction.addressee ?? ""} onChange={(event) => setField("addressee", event.target.value)} /></label>
      <label>Property address<input aria-label="Property address" value={direction.property ?? ""} onChange={(event) => setField("property", event.target.value)} /></label>
      <label>Property relevance<input aria-label="Property relevance" value={direction.propertyRelevance?.explanation ?? ""} onChange={(event) => setDirection((current) => ({
        ...current,
        propertyRelevance: event.target.value ? {
          subject: current.property ?? "",
          explanation: event.target.value,
        } : null,
      }))} /></label>
      <label>Account<input aria-label="Account" value={direction.account ?? ""} onChange={(event) => setField("account", event.target.value)} /></label>
      <label>Amount (optional)<input aria-label="Amount" value={direction.amount ?? ""} onChange={(event) => setField("amount", event.target.value)} /></label>
      <label className="wide-field">Relevant dates<input aria-label="Relevant dates" value={datesDraft} onChange={(event) => setDatesDraft(event.target.value)} placeholder="YYYY-MM-DD, YYYY-MM-DD" /></label>
      <button type="submit">Save Household Context</button>
    </form>}
    {(arrival.processingState === "needsMemberDirection" || arrival.processingState === "waitingForConnectivity") && unresolvedFields.length > 0 && <section className="cloud-assistance-inline" aria-label="Cloud assistance for this document">
      <div className="cloud-assistance-inline-heading">
        <div><strong>Need help with these fields?</strong><small>Luna can ask a connected provider for suggestions. You review them before saving or filing.</small></div>
        {!cloudOpen && <button type="button" onClick={() => void openCloudAssistance()}>{arrival.processingState === "waitingForConnectivity" ? "Retry provider assistance" : "Ask a provider"}</button>}
      </div>
      {cloudOpen && <div className="cloud-assistance-inline-panel">
        {cloudBusy && <p className="muted">Preparing provider consent…</p>}
        {!cloudBusy && <>
          <label>Provider<select value={cloudProviderId} onChange={(event) => setCloudProviderId(event.target.value)}>
            {cloudProviders.map(({ descriptor, configured }) => <option key={descriptor.id} value={descriptor.id}>{descriptor.name}{configured ? " · connected" : " · not connected"}</option>)}
          </select></label>
          {!selectedCloudProvider?.configured && <p className="muted">Connect this provider in Options before allowing it to evaluate a document.</p>}
          <div className="cloud-assistance-inline-actions">
            {existingCloudScope && <button type="button" disabled={cloudBusy || !selectedCloudProvider?.configured} onClick={() => void askCloudProvider("useExistingScope")}>Use existing consent</button>}
            <button type="button" disabled={cloudBusy || !selectedCloudProvider?.configured} onClick={() => void askCloudProvider("allowOnce")}>Allow once</button>
            <button type="button" disabled={cloudBusy || !selectedCloudProvider?.configured} onClick={() => void askCloudProvider("allowForScope")}>Allow future evaluations</button>
            <button type="button" disabled={cloudBusy} onClick={() => cloudProviderId ? void askCloudProvider("keepLocal") : setCloudMessage("Kept local. No document data was sent to a provider.")}>Keep local</button>
            <button type="button" onClick={() => setCloudOpen(false)}>Close</button>
          </div>
        </>}
        {cloudMessage && <p className="cloud-assistance-inline-message">{cloudMessage}</p>}
        {cloudError && <p role="alert" className="error">{cloudError}</p>}
      </div>}
    </section>}
    {clarificationQuestions.length > 0 && <div className="clarification-questions">
      <small>Luna still needs to know</small>
      {clarificationQuestions.map((question) => <p key={question.field}>{question.prompt}</p>)}
    </div>}
    {decision && !decision.confirmed && <form className="filing-decision-form" onSubmit={(event) => {
      event.preventDefault();
      void onConfirm({ fileName, cabinetDestination });
    }}>
      <label>Proposed filename<input aria-label="Proposed filename" value={fileName} onChange={(event) => setFileName(event.target.value)} /></label>
      <label>Cabinet Destination<input aria-label="Cabinet Destination" value={cabinetDestination} onChange={(event) => setCabinetDestination(event.target.value)} /></label>
      <button type="submit">Confirm Filing Decision</button>
    </form>}
    {arrival.filedOriginal
      ? <p className="confirmed-destination">Filed at: {arrival.filedOriginal.filingDecision.cabinetDestination}</p>
      : decision?.confirmed && <p className="confirmed-destination">Confirmed destination: {decision.cabinetDestination}</p>}
    {arrival.reviewCard.learnedRule && <aside className="learned-rule" aria-label="Learned filing rule">
      <small>Learned filing rule</small>
      <p>For {arrival.reviewCard.learnedRule.documentType} from {arrival.reviewCard.learnedRule.serviceProvider} addressed to {arrival.reviewCard.learnedRule.addressee}{arrival.reviewCard.learnedRule.property ? ` at ${arrival.reviewCard.learnedRule.property}` : ""}{arrival.reviewCard.learnedRule.account ? ` on account ${arrival.reviewCard.learnedRule.account}` : ""}, Luna can file an exact match at {arrival.reviewCard.learnedRule.cabinetDestination}.</p>
    </aside>}
  </section>;
}

export function ConversationWorkspace({
  conversationService,
  destination,
  householdId,
  cabinetRecoveryRequest,
  newConversationRequest,
  conversationSelectionRequest,
  onRecentConversationsChange,
  onActiveConversationChange,
  householdName,
  onOpenConversation,
  onTodoCountChange,
}: ConversationWorkspaceProps) {
  const [conversations, setConversations] = useState<Conversation[]>([]);
  const [messages, setMessages] = useState<ConversationMessage[]>([]);
  const [arrivals, setArrivals] = useState<DocumentArrival[]>([]);
  const [todos, setTodos] = useState<TodoItem[]>([]);
  const [selectedConversationId, setSelectedConversationId] = useState<number | null>(null);
  const [draft, setDraft] = useState("");
  const [search, setSearch] = useState("");
  const [includeArchived, setIncludeArchived] = useState(false);
  const [renaming, setRenaming] = useState(false);
  const [titleDraft, setTitleDraft] = useState("");
  const [actionsOpen, setActionsOpen] = useState(false);
  const [focusedArrivalId, setFocusedArrivalId] = useState<number | null>(null);
  const [error, setError] = useState("");
  const [dropReady, setDropReady] = useState(false);
  const initialized = useRef(false);
  const lastNewRequest = useRef(newConversationRequest);

  const selectedConversation = conversations.find(({ id }) => id === selectedConversationId) ?? null;
  const selectedArrivals = arrivals
    .filter(({ conversationId }) => conversationId === selectedConversationId)
    .sort((left, right) => left.id - right.id);

  useEffect(() => {
    onActiveConversationChange(selectedConversationId);
  }, [onActiveConversationChange, selectedConversationId]);

  const loadHouseholdWork = useCallback(async (preserveDeletedConversationId?: number) => {
    const [loadedConversations, loadedArrivals, loadedTodos] = await Promise.all([
      conversationService.listConversations(householdId, search, includeArchived),
      conversationService.listDocumentArrivals(householdId),
      conversationService.listTodoItems(householdId),
    ]);
    setConversations(loadedConversations);
    if (!search) onRecentConversationsChange(loadedConversations.filter(({ archived }) => !archived));
    setArrivals(loadedArrivals);
    setTodos(loadedTodos);
    onTodoCountChange(loadedTodos.length);
    setSelectedConversationId((current) => {
      if (preserveDeletedConversationId && loadedArrivals.some(
        ({ conversationId }) => conversationId === preserveDeletedConversationId,
      )) return preserveDeletedConversationId;
      if (current && (
        loadedConversations.some(({ id }) => id === current)
        || loadedArrivals.some(({ id, conversationId }) => (
          id === focusedArrivalId && conversationId === current
        ))
      )) return current;
      return loadedConversations[0]?.id ?? null;
    });
    return loadedArrivals;
  }, [conversationService, focusedArrivalId, householdId, includeArchived, onRecentConversationsChange, onTodoCountChange, search]);

  const createConversation = useCallback(async () => {
    const created = await conversationService.createConversation(householdId, "New conversation");
    const [loadedConversations, loadedArrivals, loadedTodos] = await Promise.all([
      conversationService.listConversations(householdId, undefined, false),
      conversationService.listDocumentArrivals(householdId),
      conversationService.listTodoItems(householdId),
    ]);
    setSearch("");
    setIncludeArchived(false);
    setConversations(loadedConversations);
    onRecentConversationsChange(loadedConversations);
    setArrivals(loadedArrivals);
    setTodos(loadedTodos);
    onTodoCountChange(loadedTodos.length);
    setSelectedConversationId(created.id);
    setFocusedArrivalId(null);
    onOpenConversation();
  }, [conversationService, householdId, onOpenConversation, onRecentConversationsChange, onTodoCountChange]);

  const deleteConversationAndRecoverWorkspace = useCallback(async (conversationId: number) => {
    try {
      await conversationService.deleteConversation(householdId, conversationId);
      const remainingConversations = await conversationService.listConversations(
        householdId,
        undefined,
        false,
      );
      if (remainingConversations.length === 0) {
        await createConversation();
      } else {
        setSearch("");
        setIncludeArchived(false);
        await loadHouseholdWork();
      }
    } catch {
      setError("Luna could not delete that Conversation.");
    }
  }, [conversationService, createConversation, householdId, loadHouseholdWork]);

  useEffect(() => {
    if (initialized.current) return;
    initialized.current = true;
    void conversationService.resumeDocumentFilings(householdId)
      .then(() => conversationService.listConversations(householdId, undefined, false))
      .then(async (loaded) => {
        if (loaded.length === 0) await createConversation();
        else {
          setConversations(loaded);
          onRecentConversationsChange(loaded);
          const requestedId = conversationSelectionRequest?.conversationId;
          setSelectedConversationId(
            requestedId && loaded.some(({ id }) => id === requestedId)
              ? requestedId
              : loaded[0].id,
          );
          const [loadedArrivals, loadedTodos] = await Promise.all([
            conversationService.listDocumentArrivals(householdId),
            conversationService.listTodoItems(householdId),
          ]);
          setArrivals(loadedArrivals);
          setTodos(loadedTodos);
          onTodoCountChange(loadedTodos.length);
        }
      })
      .catch(() => setError("Luna could not open this Household's Conversations."));
  }, [conversationSelectionRequest, conversationService, createConversation, householdId, onRecentConversationsChange, onTodoCountChange]);

  useEffect(() => {
    if (!conversationSelectionRequest) return;
    setSearch("");
    setIncludeArchived(false);
    setFocusedArrivalId(null);
    setSelectedConversationId(conversationSelectionRequest.conversationId);
  }, [conversationSelectionRequest]);

  useEffect(() => {
    if (!initialized.current) return;
    void loadHouseholdWork().catch(() => setError("Luna could not refresh the Conversation list."));
  }, [includeArchived, loadHouseholdWork, search]);

  useEffect(() => {
    if (!initialized.current || cabinetRecoveryRequest === 0) return;
    void conversationService.resumeDocumentFilings(householdId)
      .then(() => loadHouseholdWork())
      .catch(() => setError("Luna could not retry staged Cabinet work."));
  }, [cabinetRecoveryRequest, conversationService, householdId, loadHouseholdWork]);

  useEffect(() => {
    if (newConversationRequest === lastNewRequest.current) return;
    lastNewRequest.current = newConversationRequest;
    void createConversation().catch(() => setError("Luna could not create a Conversation."));
  }, [createConversation, newConversationRequest]);

  useEffect(() => {
    if (!selectedConversationId) {
      setMessages([]);
      return;
    }
    void conversationService.listMessages(householdId, selectedConversationId)
      .then(setMessages)
      .catch(() => setError("Luna could not open the Conversation messages."));
  }, [conversationService, householdId, selectedConversationId]);

  const attachPaths = useCallback(async (paths: string[]) => {
    if (!selectedConversationId) return;
    try {
      for (const path of paths) {
        await conversationService.attachDocument(householdId, selectedConversationId, path);
      }
      setError("");
      const loadedArrivals = await loadHouseholdWork();
      const conversationArrivals = loadedArrivals.filter(
        ({ conversationId }) => conversationId === selectedConversationId,
      );
      const newestArrival = [...conversationArrivals].sort((left, right) => right.id - left.id)[0];
      setFocusedArrivalId(newestArrival?.id ?? null);
    } catch (attachmentError) {
      setError(String(attachmentError));
    }
  }, [conversationService, householdId, loadHouseholdWork, selectedConversationId]);

  useEffect(() => {
    let disposed = false;
    let unlisten: (() => void) | undefined;
    setDropReady(false);
    void getCurrentWebview().onDragDropEvent((event) => {
      if (event.payload.type === "drop") void attachPaths(event.payload.paths);
    }).then((stop) => {
      if (disposed) {
        stop();
        return;
      }
      unlisten = stop;
      setDropReady(true);
    });
    return () => {
      disposed = true;
      unlisten?.();
    };
  }, [attachPaths]);

  useEffect(() => {
    if (destination !== "Luna" || focusedArrivalId === null) return;
    const item = document.querySelector<HTMLElement>(
      `.document-arrival[data-arrival-id="${focusedArrivalId}"]`,
    );
    item?.focus();
    item?.scrollIntoView({ block: "center" });
  }, [destination, focusedArrivalId, selectedArrivals.length]);

  const submitMessage = async (event: FormEvent<HTMLFormElement>) => {
    event.preventDefault();
    if (!selectedConversationId || !draft.trim()) return;
    try {
      const message = await conversationService.addMemberMessage(householdId, selectedConversationId, draft);
      setMessages((current) => [...current, message]);
      setDraft("");
    } catch {
      setError("Luna could not save that message.");
    }
  };

  const dismissArrival = async (arrivalId: number) => {
    try {
      await conversationService.dismissDocumentArrival(householdId, arrivalId);
      setFocusedArrivalId((current) => current === arrivalId ? null : current);
      await loadHouseholdWork();
    } catch {
      setError("Luna could not update that Document Handling.");
    }
  };

  const recordDirection = async (
    arrivalId: number,
    direction: DocumentContextDirection,
  ) => {
    try {
      await conversationService.recordMemberDirection(householdId, arrivalId, direction);
      setError("");
      await loadHouseholdWork();
    } catch (directionError) {
      setError(String(directionError));
    }
  };

  const confirmDecision = async (
    arrivalId: number,
    direction: FilingDecisionDirection,
  ) => {
    try {
      await conversationService.confirmFilingDecision(householdId, arrivalId, direction);
      setError("");
      setFocusedArrivalId(null);
      await loadHouseholdWork();
    } catch (decisionError) {
      setError(String(decisionError));
    }
  };

  const resolveDuplicate = async (
    arrivalId: number,
    relatedArrivalId: number,
    decision: DuplicateDecision,
    rememberPreference: boolean,
  ) => {
    try {
      await conversationService.resolveDuplicate(
        householdId,
        arrivalId,
        relatedArrivalId,
        decision,
        rememberPreference,
      );
      setError("");
      await loadHouseholdWork();
    } catch (duplicateError) {
      setError(String(duplicateError));
    }
  };

  const openTodo = async (todo: TodoItem) => {
    try {
      const availableConversations = await conversationService.listConversations(
        householdId,
        undefined,
        true,
      );
      setSearch("");
      setIncludeArchived(true);
      setConversations(availableConversations);
      onRecentConversationsChange(availableConversations.filter(({ archived }) => !archived));
      setSelectedConversationId(todo.conversationId);
      setFocusedArrivalId(todo.arrivalId);
      onOpenConversation();
    } catch {
      setError("Luna could not open that Conversation item.");
    }
  };

  if (destination === "To do") {
    return <main className="conversation todo-view">
      <header><div><small>Attention</small><h1>To do</h1></div><span>{todos.length} requiring direction</span></header>
      {error && <p role="alert" className="session-notice">{error}</p>}
      <section className="todo-list" aria-label="To-do Items">
        {todos.length === 0 && <p className="empty-state">Nothing needs your attention.</p>}
        {todos.map((todo) => <article key={todo.arrivalId} data-arrival-id={todo.arrivalId}>
          <div><small>{todo.conversationTitle}</small><h2>{todo.documentName}</h2><p>{todo.processingState === "possibleDuplicate" ? "Needs duplicate decision" : todo.processingState === "cabinetUnavailable" ? "Waiting for Cabinet" : "Needs your direction"}</p></div>
          <div>
            <button type="button" onClick={() => void openTodo(todo)}>Open Conversation item</button>
            {todo.processingState === "needsMemberDirection" && <button type="button" onClick={() => void dismissArrival(todo.arrivalId)}>Dismiss</button>}
          </div>
        </article>)}
      </section>
    </main>;
  }

  return <main className="conversation conversation-workspace">
    <header className="conversation-header">
      <div className="conversation-heading">
        <span className="conversation-folder" aria-hidden="true" />
        <div className="conversation-title-wrap">
          {renaming && selectedConversation ? <form onSubmit={(event) => {
            event.preventDefault();
            void conversationService.renameConversation(householdId, selectedConversation.id, titleDraft)
              .then(async () => {
                setRenaming(false);
                await loadHouseholdWork();
              })
              .catch(() => setError("Luna could not rename that Conversation."));
          }}>
            <input aria-label="Conversation title" value={titleDraft} onChange={(event) => setTitleDraft(event.target.value)} />
            <button type="submit">Save title</button>
          </form> : <h1 className="conversation-title">{selectedConversation?.title ?? (selectedArrivals.length > 0 ? "Deleted Conversation" : "Conversations")}</h1>}
        </div>
      </div>
      <div className="conversation-header-meta">
        <span className="conversation-privacy"><span className="conversation-privacy-dot" aria-hidden="true" />Private</span>
        <button
          type="button"
          className="conversation-actions-trigger"
          aria-label="Conversation actions"
          aria-expanded={actionsOpen}
          onClick={() => setActionsOpen((current) => !current)}
        >
          <span aria-hidden="true">•••</span>
        </button>
        {actionsOpen && <div className="conversation-actions-popover" aria-label="Conversation actions menu">
          <div className="conversation-household-context"><small>Household</small><strong>{householdName}</strong></div>
          <input aria-label="Search Conversations" placeholder="Search Conversations" value={search} onChange={(event) => {
            setFocusedArrivalId(null);
            setSearch(event.target.value);
          }} />
          <label className="conversation-actions-checkbox"><input type="checkbox" checked={includeArchived} onChange={(event) => setIncludeArchived(event.target.checked)} /> Show archived</label>
          <select aria-label="Conversations" value={selectedConversationId ?? ""} onChange={(event) => {
            setFocusedArrivalId(null);
            setSelectedConversationId(Number(event.target.value));
          }}>
            {conversations.map((conversation) => <option key={conversation.id} value={conversation.id}>{conversation.title}{conversation.archived ? " (archived)" : ""}</option>)}
          </select>
          {selectedConversation && <div className="conversation-actions-buttons">
            <button type="button" onClick={() => {
              setTitleDraft(selectedConversation.title);
              setRenaming(true);
              setActionsOpen(false);
            }}>Rename</button>
            <button type="button" onClick={() => {
              setActionsOpen(false);
              void conversationService.archiveConversation(householdId, selectedConversation.id, !selectedConversation.archived).then(() => loadHouseholdWork());
            }}>
              {selectedConversation.archived ? "Restore" : "Archive"}
            </button>
            <button type="button" className="conversation-delete-action" onClick={() => {
              setActionsOpen(false);
              void deleteConversationAndRecoverWorkspace(selectedConversation.id);
            }}>Delete</button>
          </div>}
        </div>}
      </div>
    </header>
    {error && <p role="alert" className="session-notice">{error}</p>}
    <section className="messages" aria-label="Conversation">
      <article className="luna-message"><span aria-hidden="true">L</span><p>What would you like me to take care of?</p></article>
      {messages.map((message) => <article className="member-message" key={message.id}><span aria-hidden="true">You</span><p>{message.body}</p></article>)}
      {selectedArrivals.map((arrival) => <article
        className="document-arrival"
        data-arrival-id={arrival.id}
        data-focused={arrival.id === focusedArrivalId ? "true" : undefined}
        key={arrival.id}
        tabIndex={-1}
      >
        <div>
          <small>Document Arrival</small><h2>{arrival.originalName}</h2><p>{stateLabel(arrival)}</p>
          {arrival.processingState === "cabinetUnavailable" && <p role="status" className="session-notice">The remembered Cabinet is unavailable. Luna kept this Original staged and will retry when the Cabinet returns.</p>}
          <DocumentReviewEditor
            arrival={arrival}
            conversationService={conversationService}
            householdId={householdId}
            onConfirm={(direction) => confirmDecision(arrival.id, direction)}
            onRecord={(direction) => recordDirection(arrival.id, direction)}
            onResolveDuplicate={(relatedArrivalId, decision, rememberPreference) => resolveDuplicate(arrival.id, relatedArrivalId, decision, rememberPreference)}
            onRefresh={async () => { await loadHouseholdWork(); }}
          />
        </div>
        {arrival.processingState === "needsMemberDirection" && <button type="button" onClick={() => void dismissArrival(arrival.id)}>Dismiss</button>}
      </article>)}
    </section>
    <div className="attachment-zone">
      <p>{dropReady
        ? "Drop a PDF, JPG, or PNG anywhere in Luna, or select a document."
        : "Preparing document drop… You can still select a document."}</p>
      <button type="button" aria-label="Attach document" disabled={!selectedConversation} onClick={() => void conversationService.selectDocumentFiles().then(attachPaths)}>Attach document</button>
    </div>
    <form className="composer" onSubmit={submitMessage}>
      <label htmlFor="message-composer">Message Luna</label>
      <textarea id="message-composer" onChange={(event) => setDraft(event.target.value)} placeholder="Message Luna or attach a document" rows={1} value={draft} />
      <button type="submit" aria-label="Send message">↑</button>
    </form>
  </main>;
}
