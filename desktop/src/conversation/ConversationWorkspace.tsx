import {
  FormEvent,
  Fragment,
  useCallback,
  useEffect,
  useRef,
  useState,
} from "react";
import { getCurrentWebview } from "@tauri-apps/api/webview";
import type {
  Conversation,
  ConversationIntelligenceFailure,
  ConversationMessage,
  ConversationService,
  CloudConsentScope,
  DefaultIntelligenceProvider,
  ConversationAction,
  DocumentConversationView,
  DocumentContextDirection,
  DocumentArrival,
  DocumentProcessingState,
  DuplicateDecision,
  FilingDecisionDirection,
  IntelligenceResult,
  IntelligenceSelection,
  IntelligenceProviderStatus,
  TodoItem,
} from "./conversationService";
import {
  cloudConsentScopeGrantsDefaultPermission,
} from "./conversationService";

const intelligenceFailureNotice = (
  failure: ConversationIntelligenceFailure | null,
): string => {
  if (!failure) return "";
  const provider = failure.providerName ?? "The selected Intelligence Provider";
  switch (failure.code) {
    case "not_configured":
    case "configuration":
    case "consent_required":
      return `${failure.detail} Open Options → Cloud assistance to review the default and Conversation permission.`;
    case "unavailable":
      return `${provider} could not complete the request. Your message remains saved in Luna, and no Intelligence Provider reply was added. The request may have reached ${provider}; check the connection and try again.`;
    case "invalid_result":
      return `${provider} returned a reply Luna could not safely accept. Your message remains saved, and no Intelligence Provider reply was added.`;
    case "invalid_credential":
    case "request_rejected":
      return `${provider} rejected the request. Your message remains saved, and no Intelligence Provider reply was added. Review the connection in Options.`;
    default:
      return "Luna could not complete the Intelligence Provider request. Your message remains saved, and no Intelligence Provider reply was added.";
  }
};

type ConversationWorkspaceProps = {
  conversationService: ConversationService;
  destination: "Luna" | "To do";
  householdId: string;
  cabinetRecoveryRequest: number;
  newConversationRequest: number;
  conversationSelectionRequest: { conversationId: number; request: number } | null;
  onRecentConversationsChange(conversations: Conversation[]): void;
  onActiveConversationChange(conversationId: number | null): void;
  onCabinetUnavailable(): void;
  householdName: string;
  onOpenConversation(): void;
  onTodoCountChange(count: number): void;
};

const stateLabel = (arrival: DocumentArrival) => ({
  needsCloudConsent: "Needs Cloud Assistance choice",
  inspectingWithAssistance: "Inspecting with Cloud Assistance",
  waitingForCloudAssistance: "Waiting to retry Cloud Assistance",
  needsMemberDirection: "Needs your direction",
  possibleDuplicate: "Needs duplicate decision",
  readyToFile: "Ready to file",
  filing: "Filing",
  cabinetUnavailable: "Waiting for Cabinet",
  filed: "Filed",
  dismissed: "Dismissed",
})[arrival.processingState];

const canDismiss = (processingState: DocumentProcessingState) => (
  processingState === "needsCloudConsent"
  || processingState === "waitingForCloudAssistance"
  || processingState === "needsMemberDirection"
);

const confidenceLabel = (arrival: DocumentArrival) => ({
  confirmed: "Confirmed",
  looksRight: "Looks right",
  needsChecking: "Needs checking",
  unknown: "Unknown",
})[arrival.reviewCard.confidenceState];

const authorityLabel = (arrival: DocumentArrival) => {
  if (arrival.authoritySource === "filingRule") return "Scoped Filing Rule";
  if (arrival.authoritySource === "memberDirection") return "Member Direction";
  return "Member Direction required before filing";
};

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

const conversationActionLabels: Record<Exclude<ConversationAction, "reviewDetails">, string> = {
  yes: "Yes",
  no: "No",
  keepLocal: "Keep local",
  keepBoth: "Keep both",
  linkCopies: "Link copies",
  discardNew: "Discard new",
  updatedVersion: "Updated version",
  alwaysDoThis: "Always do this",
};

type DocumentReviewEditorProps = {
  arrival: DocumentArrival;
  conversationService: ConversationService;
  householdId: string;
  cloudConversationOutcome?: {
    result: IntelligenceResult | null;
    response: string;
  };
  onConsumeCloudConversationOutcome(arrivalId: number): void;
  onConfirm(direction: FilingDecisionDirection): Promise<void>;
  onRecord(direction: DocumentContextDirection): Promise<void>;
  onResolveDuplicate(relatedArrivalId: number, decision: DuplicateDecision, rememberPreference: boolean): Promise<void>;
  onRegisterCloudConversationHandler(
    arrivalId: number,
    binding: {
      selection: IntelligenceSelection | null;
      existingConsentGrantId: number | null;
    } | null,
  ): void;
  onRefresh(): Promise<void>;
};

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
  cloudConversationOutcome,
  onConsumeCloudConversationOutcome,
  onConfirm,
  onRecord,
  onResolveDuplicate,
  onRegisterCloudConversationHandler,
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
  const [cloudDefaultProvider, setCloudDefaultProvider] = useState<DefaultIntelligenceProvider | null>(null);
  const [cloudBusy, setCloudBusy] = useState(false);
  const [cloudMessage, setCloudMessage] = useState("");
  const [cloudError, setCloudError] = useState("");
  const [cloudReadyForMemberDirection, setCloudReadyForMemberDirection] = useState(false);
  const [cloudSuggestion, setCloudSuggestion] = useState<{
    requestId: string;
    fields: Record<string, string>;
  } | null>(null);
  const clarificationQuestions = (
    arrival.processingState === "needsMemberDirection"
    || cloudReadyForMemberDirection
  )
    ? arrival.reviewCard.questions.filter(({ field }) => field !== "amount")
    : arrival.reviewCard.questions;
  const selectedCloudProvider = cloudProviders.find(({ descriptor }) => (
    !cloudDefaultProvider?.invalid
    && descriptor.id === cloudDefaultProvider?.providerId
  ));
  const selectedCloudModel = selectedCloudProvider?.descriptor.models.find(
    ({ id }) => id === cloudDefaultProvider?.modelId,
  );
  const existingCloudScope = cloudScopes.find((scope) => (
    cloudConsentScopeGrantsDefaultPermission(scope, cloudDefaultProvider, "document")
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
    setCloudDefaultProvider(null);
    setCloudMessage("");
    setCloudError("");
    setCloudReadyForMemberDirection(false);
    setCloudSuggestion(null);
  }, [arrival.id]);

  const openCloudAssistance = async () => {
    setCloudOpen(true);
    setCloudBusy(true);
    setCloudError("");
    try {
      const [providers, scopes, defaultProvider] = await Promise.all([
        conversationService.listIntelligenceProviderStatuses(householdId),
        conversationService.listCloudConsentScopes(householdId),
        conversationService.getDefaultIntelligenceProvider(householdId),
      ]);
      setCloudProviders(providers);
      setCloudScopes(scopes.scopes);
      setCloudDefaultProvider(defaultProvider);
    } catch (reason) {
      setCloudError(String(reason));
    } finally {
      setCloudBusy(false);
    }
  };

  useEffect(() => {
    if (
      (arrival.processingState === "needsCloudConsent"
        || arrival.processingState === "waitingForCloudAssistance")
      && !cloudOpen
    ) {
      void openCloudAssistance();
    }
  }, [arrival.processingState]);

  const askDefaultCloudProvider = useCallback(async () => {
    setCloudBusy(true);
    setCloudError("");
    setCloudMessage("");
    try {
      const outcome = await conversationService.evaluateDocumentWithDefaultIntelligenceProvider(
        householdId,
        arrival.id,
      );
      await onRefresh();
      if (!outcome.result) {
        setCloudReadyForMemberDirection(true);
        const response = "Kept local. No document information was sent to an Intelligence Provider.";
        setCloudMessage(response);
        return response;
      }
      const result = outcome.result;
      const suggestedFields = Object.keys(result.fields);
      setCloudReadyForMemberDirection(true);
      if (suggestedFields.length === 0) {
        const response = `${selectedCloudProvider?.descriptor.name ?? "The Intelligence Provider"} returned no usable suggestions. Luna kept this review ready for your direction.`;
        setCloudMessage(response);
        return response;
      } else {
        setDirection((current) => applyCloudFields(current, result.fields));
        setCloudSuggestion({ requestId: result.requestId, fields: result.fields });
        const response = `${selectedCloudProvider?.descriptor.name ?? "The Intelligence Provider"} ${selectedCloudModel?.name ?? ""} suggested ${suggestedFields.join(", ")}. This is untrusted Evidence; review it before saving Household Context.`;
        setCloudMessage(response);
        return response;
      }
    } catch (reason) {
      const response = String(reason);
      setCloudError(response);
      await onRefresh();
      return response;
    } finally {
      setCloudBusy(false);
    }
  }, [
    arrival.id,
    conversationService,
    householdId,
    onRefresh,
    selectedCloudModel?.name,
    selectedCloudProvider?.descriptor.name,
  ]);

  useEffect(() => {
    onRegisterCloudConversationHandler(arrival.id, {
      selection: cloudDefaultProvider && !cloudDefaultProvider.invalid
        ? {
          providerId: cloudDefaultProvider.providerId,
          modelId: cloudDefaultProvider.modelId,
        }
        : null,
      existingConsentGrantId: existingCloudScope?.id ?? null,
    });
    return () => onRegisterCloudConversationHandler(arrival.id, null);
  }, [
    arrival.id,
    cloudDefaultProvider,
    conversationService,
    existingCloudScope,
    householdId,
    onRegisterCloudConversationHandler,
  ]);

  useEffect(() => {
    if (!cloudConversationOutcome) return;
    const { result, response } = cloudConversationOutcome;
    setCloudMessage(response);
    setCloudError("");
    const consumeTimer = window.setTimeout(
      () => onConsumeCloudConversationOutcome(arrival.id),
      0,
    );
    if (!result) {
      setCloudReadyForMemberDirection(response.startsWith("Kept local."));
      return () => window.clearTimeout(consumeTimer);
    }
    setCloudReadyForMemberDirection(true);
    setDirection((current) => applyCloudFields(current, result.fields));
    setCloudSuggestion({ requestId: result.requestId, fields: result.fields });
    void conversationService.listCloudConsentScopes(householdId)
      .then((listing) => setCloudScopes(listing.scopes))
      .catch((reason: unknown) => setCloudError(String(reason)));
    return () => window.clearTimeout(consumeTimer);
  }, [
    arrival.id,
    cloudConversationOutcome,
    conversationService,
    householdId,
    onConsumeCloudConversationOutcome,
  ]);

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

  const saveDirection = async () => {
    const submitted: DocumentContextDirection = {
      ...direction,
      relevantDates: datesDraft.split(",").map((date) => date.trim()).filter(Boolean),
      documentTypeResolved: true,
      serviceProviderResolved: true,
      addresseeResolved: true,
      propertyResolved: true,
      accountResolved: true,
      amountResolved: true,
      relevantDatesResolved: true,
    };
    try {
      await onRecord(submitted);
    } catch (reason) {
      setCloudError(String(reason));
      return;
    }
    if (cloudSuggestion) {
      const submittedValues: Record<string, string> = {
        documentType: submitted.documentType ?? "",
        serviceProvider: submitted.serviceProvider ?? "",
        addressee: submitted.addressee ?? "",
        property: submitted.property ?? "",
        account: submitted.account ?? "",
        amount: submitted.amount ?? "",
        relevantDates: submitted.relevantDates.join(", "),
      };
      const accepted = Object.entries(cloudSuggestion.fields).every(
        ([field, value]) => submittedValues[field]?.trim() === value.trim(),
      );
      await conversationService.recordCloudCandidateDisposition(
        householdId,
        arrival.id,
        cloudSuggestion.requestId,
        accepted ? "accepted" : "corrected",
      );
      setCloudSuggestion(null);
    }
  };

  const cloudAssistance = (
    arrival.processingState === "needsCloudConsent"
    || arrival.processingState === "waitingForCloudAssistance"
    || (arrival.processingState === "needsMemberDirection" && arrival.reviewCard.questions.length > 0)
  ) && <section className="cloud-assistance-inline" aria-label="Cloud assistance for this document">
    <div className="cloud-assistance-inline-heading">
      <div><strong>Local Evidence is not enough to interpret this Document safely.</strong><small>Cloud Assistance can suggest unresolved fields. It cannot create Member Direction, file the Original, or change a Filing Rule.</small></div>
      {!cloudOpen && <button type="button" onClick={() => void openCloudAssistance()}>Review Cloud Assistance</button>}
    </div>
    {cloudOpen && <div className="cloud-assistance-inline-panel">
      {cloudBusy && <p className="muted">Preparing the default Intelligence Provider…</p>}
      {!cloudBusy && <>
        {selectedCloudProvider && selectedCloudModel
          ? <p><strong>Default: {selectedCloudProvider.descriptor.name} {selectedCloudModel.name}.</strong> Luna will send only the approved Document fields: media type, the displayed context values, relevant dates, and at most 4,000 characters of locally extracted text. Cabinet paths, Household state, credentials, Filing Rules, prior Conversations, and the Original file are not sent.</p>
          : <p className="muted">Choose a default Intelligence Provider and model in Options → Cloud assistance.</p>}
        {selectedCloudProvider && selectedCloudModel && !existingCloudScope
          && <p className="muted">Enable Document evaluations by default in Options → Cloud assistance.</p>}
        {selectedCloudProvider && !selectedCloudProvider.configured
          && <p className="muted">The default Intelligence Provider is not available on this Trusted Device. Review Cloud assistance in Options.</p>}
        <div className="cloud-assistance-inline-actions">
          <button
            type="button"
            disabled={cloudBusy || !selectedCloudProvider?.configured || !selectedCloudModel || !existingCloudScope}
            onClick={() => void askDefaultCloudProvider()}
          >Ask default Intelligence Provider</button>
          <button type="button" onClick={() => setCloudOpen(false)}>Close</button>
        </div>
      </>}
      {cloudMessage && <p className="cloud-assistance-inline-message">{cloudMessage}</p>}
      {cloudError && <p role="alert" className="error">{cloudError}</p>}
    </div>}
  </section>;

  const duplicateHandling = <>
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
  </>;

  return <>{duplicateHandling}{cloudAssistance}<details className="review-details">
    <summary>Review details</summary>
    <section className="review-card" aria-label={`Review details for ${arrival.originalName}`}>
    <strong>{confidenceLabel(arrival)}</strong>
    <dl className="review-transparency">
      <div><dt>Authority</dt><dd>{authorityLabel(arrival)}</dd></div>
      <div><dt>Relevant consent</dt><dd>{arrival.cloudAssistanceHistory.length > 0
        ? <ul>{arrival.cloudAssistanceHistory.map((entry) => <li key={entry}>{entry}</li>)}</ul>
        : "No Cloud Assistance consent recorded; inspection remained local."}</dd></div>
      <div><dt>Execution history</dt><dd><ol>{arrival.executionHistory.map((entry) => <li key={entry}>{entry}</li>)}</ol></dd></div>
    </dl>
    <dl>{arrival.reviewCard.evidence.map((evidence) => <div key={evidence.label}>
      <dt>{evidence.label}</dt><dd>{evidence.value}</dd>
    </div>)}</dl>
    {(
      arrival.processingState === "needsMemberDirection"
      || arrival.processingState === "needsCloudConsent"
      || arrival.processingState === "waitingForCloudAssistance"
      || cloudReadyForMemberDirection
    ) && <form className="context-review-form" onSubmit={(event) => {
      event.preventDefault();
      void saveDirection();
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
    </section>
  </details></>;
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
  onCabinetUnavailable,
  householdName,
  onOpenConversation,
  onTodoCountChange,
}: ConversationWorkspaceProps) {
  const [conversations, setConversations] = useState<Conversation[]>([]);
  const [messages, setMessages] = useState<ConversationMessage[]>([]);
  const [arrivals, setArrivals] = useState<DocumentArrival[]>([]);
  const [documentConversations, setDocumentConversations] = useState<Record<number, DocumentConversationView>>({});
  const [turnMessages, setTurnMessages] = useState<Record<number, string>>({});
  const [cloudTurnOutcomes, setCloudTurnOutcomes] = useState<Record<number, {
    result: IntelligenceResult | null;
    response: string;
  }>>({});
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
  const [intelligenceNotice, setIntelligenceNotice] = useState("");
  const [dropReady, setDropReady] = useState(false);
  const initialized = useRef(false);
  const lastNewRequest = useRef(newConversationRequest);
  const cloudConversationHandlers = useRef(new Map<
    number,
    {
      selection: IntelligenceSelection | null;
      existingConsentGrantId: number | null;
    }
  >());

  const selectedConversation = conversations.find(({ id }) => id === selectedConversationId) ?? null;
  const selectedArrivals = arrivals
    .filter(({ conversationId }) => conversationId === selectedConversationId)
    .sort((left, right) => left.id - right.id);
  const consumeCloudTurnOutcome = useCallback((arrivalId: number) => {
    setCloudTurnOutcomes((current) => {
      if (!(arrivalId in current)) return current;
      const next = { ...current };
      delete next[arrivalId];
      return next;
    });
  }, []);

  const loadDocumentConversations = useCallback(async (loadedArrivals: DocumentArrival[]) => {
    const entries = await Promise.all(loadedArrivals.map(async (arrival) => [
      arrival.id,
      await conversationService.getDocumentConversation(householdId, arrival.id),
    ] as const));
    setDocumentConversations(Object.fromEntries(entries));
  }, [conversationService, householdId]);

  useEffect(() => {
    onActiveConversationChange(selectedConversationId);
  }, [onActiveConversationChange, selectedConversationId]);

  const loadHouseholdWork = useCallback(async (options: {
    preserveDeletedConversationId?: number;
    search?: string;
    includeArchived?: boolean;
    selectConversationId?: number;
  } = {}) => {
    const effectiveSearch = options.search ?? search;
    const effectiveIncludeArchived = options.includeArchived ?? includeArchived;
    const [loadedConversations, loadedArrivals, loadedTodos] = await Promise.all([
      conversationService.listConversations(
        householdId,
        effectiveSearch,
        effectiveIncludeArchived,
      ),
      conversationService.listDocumentArrivals(householdId),
      conversationService.listTodoItems(householdId),
    ]);
    setConversations(loadedConversations);
    if (!effectiveSearch) {
      onRecentConversationsChange(loadedConversations.filter(({ archived }) => !archived));
    }
    setArrivals(loadedArrivals);
    await loadDocumentConversations(loadedArrivals);
    setTodos(loadedTodos);
    onTodoCountChange(loadedTodos.length);
    setSelectedConversationId((current) => {
      if (options.selectConversationId && loadedConversations.some(
        ({ id }) => id === options.selectConversationId,
      )) return options.selectConversationId;
      if (options.preserveDeletedConversationId && loadedArrivals.some(
        ({ conversationId }) => conversationId === options.preserveDeletedConversationId,
      )) return options.preserveDeletedConversationId;
      if (current && (
        loadedConversations.some(({ id }) => id === current)
        || loadedArrivals.some(({ id, conversationId }) => (
          id === focusedArrivalId && conversationId === current
        ))
      )) return current;
      return loadedConversations[0]?.id ?? null;
    });
    return { conversations: loadedConversations, arrivals: loadedArrivals };
  }, [conversationService, focusedArrivalId, householdId, includeArchived, loadDocumentConversations, onRecentConversationsChange, onTodoCountChange, search]);

  const createConversation = useCallback(async () => {
    const created = await conversationService.createConversation(householdId, "New conversation");
    setSearch("");
    setIncludeArchived(false);
    await loadHouseholdWork({
      search: "",
      includeArchived: false,
      selectConversationId: created.id,
    });
    setFocusedArrivalId(null);
    onOpenConversation();
  }, [conversationService, householdId, loadHouseholdWork, onOpenConversation]);

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
        await loadHouseholdWork({ search: "", includeArchived: false });
      }
    } catch {
      setError("Luna could not delete that Conversation.");
    }
  }, [conversationService, createConversation, householdId, loadHouseholdWork]);

  useEffect(() => {
    if (initialized.current) return;
    initialized.current = true;
    void conversationService.resumeDocumentFilings(householdId)
      .catch(() => {
        setError("Some Cabinet recovery work is still waiting.");
      })
      .then(async () => {
        const loaded = await loadHouseholdWork({
          search: "",
          includeArchived: false,
          selectConversationId: conversationSelectionRequest?.conversationId,
        });
        if (loaded.conversations.length === 0) await createConversation();
      })
      .catch(() => setError("Luna could not open this Household's Conversations."));
  }, [conversationSelectionRequest, conversationService, createConversation, householdId, loadHouseholdWork]);

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
      .catch(() => {
        setError("Some Cabinet recovery work is still waiting.");
      })
      .then(() => loadHouseholdWork())
      .catch(() => setError("Luna could not refresh staged Cabinet work."));
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
        const attached = await conversationService.attachDocument(
          householdId,
          selectedConversationId,
          path,
        );
        if (attached.processingState === "cabinetUnavailable") {
          onCabinetUnavailable();
        }
      }
      setError("");
      const { arrivals: loadedArrivals } = await loadHouseholdWork();
      const conversationArrivals = loadedArrivals.filter(
        ({ conversationId }) => conversationId === selectedConversationId,
      );
      const newestArrival = [...conversationArrivals].sort((left, right) => right.id - left.id)[0];
      setFocusedArrivalId(newestArrival?.id ?? null);
    } catch (attachmentError) {
      setError(String(attachmentError));
    }
  }, [
    conversationService,
    householdId,
    loadHouseholdWork,
    onCabinetUnavailable,
    selectedConversationId,
  ]);

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

  const submitUtterance = async (arrivalId: number, message: string) => {
    const prompt = documentConversations[arrivalId]?.prompt;
    if (!selectedConversationId || !prompt || !message.trim()) return;
    try {
      const cloudBinding = prompt.purpose === "chooseCloudAssistance"
        ? cloudConversationHandlers.current.get(arrivalId)
        : undefined;
      const outcome = await conversationService.submitMemberUtterance(
        householdId,
        arrivalId,
        {
          conversationId: selectedConversationId,
          message,
          linkedPrompt: prompt.id,
        },
        cloudBinding?.selection,
        cloudBinding?.existingConsentGrantId,
      );
      setTurnMessages((current) => ({
        ...current,
        [arrivalId]: prompt.purpose === "chooseCloudAssistance"
          || outcome.status === "clarificationRequired"
          || outcome.status === "actionRefused"
          ? outcome.message
          : "",
      }));
      setMessages(await conversationService.listMessages(householdId, selectedConversationId));
      setFocusedArrivalId(arrivalId);
      setError("");
      await loadHouseholdWork();
      if (prompt.purpose === "chooseCloudAssistance") {
        setCloudTurnOutcomes((current) => ({
          ...current,
          [arrivalId]: { result: outcome.cloudResult, response: outcome.message },
        }));
      }
    } catch (utteranceError) {
      setError(String(utteranceError));
    }
  };

  const submitMessage = async (event: FormEvent<HTMLFormElement>) => {
    event.preventDefault();
    if (!selectedConversationId || !draft.trim()) return;
    const messageBody = draft.trim();
    const promptedArrival = selectedArrivals.find(
      (arrival) => arrival.id === focusedArrivalId && documentConversations[arrival.id]?.prompt,
    ) ?? [...selectedArrivals]
      .reverse()
      .find((arrival) => documentConversations[arrival.id]?.prompt);
    try {
      if (promptedArrival) {
        await submitUtterance(promptedArrival.id, messageBody);
      } else {
        const submission = await conversationService.submitOrdinaryConversationMessage(
          householdId,
          selectedConversationId,
          messageBody,
        );
        setMessages((current) => [
          ...current,
          submission.memberMessage,
          ...(submission.reply ? [submission.reply] : []),
        ]);
        setIntelligenceNotice(intelligenceFailureNotice(submission.failure));
      }
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
      throw directionError;
    }
  };

  const confirmDecision = async (
    arrivalId: number,
    direction: FilingDecisionDirection,
  ) => {
    try {
      const confirmed = await conversationService.confirmFilingDecision(
        householdId,
        arrivalId,
        direction,
      );
      if (confirmed.processingState === "cabinetUnavailable") {
        onCabinetUnavailable();
      }
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
      await loadHouseholdWork().catch(() => undefined);
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

  const lastLinkedMessage = new Map<number, number>();
  for (const message of messages) {
    if (message.linkedDocumentArrival !== null) {
      lastLinkedMessage.set(message.linkedDocumentArrival, message.id);
    }
  }
  const arrivalById = new Map(selectedArrivals.map((arrival) => [arrival.id, arrival]));
  const unlinkedArrivals = selectedArrivals.filter((arrival) => !lastLinkedMessage.has(arrival.id));
  const renderDocumentArrival = (arrival: DocumentArrival) => <article
    className="document-arrival"
    data-arrival-id={arrival.id}
    data-focused={arrival.id === focusedArrivalId ? "true" : undefined}
    key={arrival.id}
    tabIndex={-1}
  >
    <div className="document-conversation">
      <div className="document-attachment">
        <small>Attached document</small>
        <strong>{arrival.originalName}</strong>
      </div>
      {arrival.processingState === "cabinetUnavailable" && <p role="status" className="session-notice">The remembered Cabinet is unavailable. Luna kept this Original staged and will retry when the Cabinet returns.</p>}
      {documentConversations[arrival.id] && <article className="luna-message document-luna-message">
        <span aria-hidden="true">L</span>
        <div>
          {turnMessages[arrival.id] && <p className="turn-message">{turnMessages[arrival.id]}</p>}
          <p className="conversation-copy">{
            documentConversations[arrival.id].prompt?.message
            ?? documentConversations[arrival.id].completionMessage
            ?? documentConversations[arrival.id].understanding
          }</p>
          {documentConversations[arrival.id].prompt && <div className="conversation-inline-actions">
            {documentConversations[arrival.id].prompt?.allowedActions
              .filter((action): action is Exclude<ConversationAction, "reviewDetails"> => action !== "reviewDetails")
              .map((action) => <button
                type="button"
                key={action}
                onClick={() => void submitUtterance(arrival.id, conversationActionLabels[action])}
              >{conversationActionLabels[action]}</button>)}
          </div>}
        </div>
      </article>}
      <DocumentReviewEditor
        arrival={arrival}
        conversationService={conversationService}
        householdId={householdId}
        cloudConversationOutcome={cloudTurnOutcomes[arrival.id]}
        onConsumeCloudConversationOutcome={consumeCloudTurnOutcome}
        onConfirm={(direction) => confirmDecision(arrival.id, direction)}
        onRecord={(direction) => recordDirection(arrival.id, direction)}
        onResolveDuplicate={(relatedArrivalId, decision, rememberPreference) => resolveDuplicate(arrival.id, relatedArrivalId, decision, rememberPreference)}
        onRegisterCloudConversationHandler={(arrivalId, handler) => {
          if (handler) cloudConversationHandlers.current.set(arrivalId, handler);
          else cloudConversationHandlers.current.delete(arrivalId);
        }}
        onRefresh={async () => { await loadHouseholdWork(); }}
      />
      {!documentConversations[arrival.id] && <p>{stateLabel(arrival)}</p>}
    </div>
    {arrival.processingState === "needsMemberDirection" && <button type="button" onClick={() => void dismissArrival(arrival.id)}>Dismiss</button>}
  </article>;

  if (destination === "To do") {
    return <main className="conversation todo-view">
      <header><div><small>Attention</small><h1>To do</h1></div><span>{todos.length} requiring attention</span></header>
      {error && <p role="alert" className="session-notice">{error}</p>}
      <section className="todo-list" aria-label="To-do Items">
        {todos.length === 0 && <p className="empty-state">Nothing needs your attention.</p>}
        {todos.map((todo) => <article key={todo.arrivalId} data-arrival-id={todo.arrivalId}>
          <div><small>{todo.conversationTitle}</small><h2>{todo.documentName}</h2><p>{todo.processingState === "possibleDuplicate" ? "Needs duplicate decision" : todo.processingState === "cabinetUnavailable" ? "Waiting for Cabinet" : "Needs your direction"}</p></div>
          <div>
            <button type="button" onClick={() => void openTodo(todo)}>Open Conversation item</button>
            {canDismiss(todo.processingState) && <button type="button" onClick={() => void dismissArrival(todo.arrivalId)}>Dismiss</button>}
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
    {intelligenceNotice && <p role="status" className="conversation-intelligence-notice">
      {intelligenceNotice}
    </p>}
    <section className="messages" aria-label="Conversation">
      <article className="luna-message"><span aria-hidden="true">L</span><p>What would you like me to take care of?</p></article>
      {messages.map((message) => <Fragment key={message.id}>
        <article className={message.author === "luna" ? "luna-message" : "member-message"}>
          <span aria-hidden="true">{message.author === "luna" ? "L" : "You"}</span>
          <p className={message.author === "luna" ? "conversation-copy" : undefined}>{message.body}</p>
        </article>
        {message.linkedDocumentArrival !== null
          && lastLinkedMessage.get(message.linkedDocumentArrival) === message.id
          && arrivalById.has(message.linkedDocumentArrival)
          && renderDocumentArrival(arrivalById.get(message.linkedDocumentArrival)!)}
      </Fragment>)}
      {unlinkedArrivals.map(renderDocumentArrival)}
    </section>
    <div className="attachment-zone">
      <p>{dropReady
        ? "Drop a PDF, JPG, or PNG anywhere in Luna, or select a document."
        : "Preparing document drop… You can still select a document."}</p>
      <button type="button" aria-label="Attach document" disabled={!selectedConversation} onClick={() => void conversationService.selectDocumentFiles().then(attachPaths)}>Attach document</button>
    </div>
    <form className="composer" onSubmit={submitMessage}>
      <label htmlFor="message-composer">Message Luna</label>
      <textarea
        aria-describedby="message-composer-hint"
        id="message-composer"
        onChange={(event) => setDraft(event.target.value)}
        onKeyDown={(event) => {
          if (event.key !== "Enter" || event.shiftKey || event.nativeEvent.isComposing) return;
          event.preventDefault();
          event.currentTarget.form?.requestSubmit();
        }}
        placeholder="Message Luna or attach a document"
        rows={1}
        value={draft}
      />
      <button type="submit" aria-label="Send message">↑</button>
      <small id="message-composer-hint">Enter to send. Shift+Enter for a new line.</small>
    </form>
  </main>;
}
