import { invoke } from "@tauri-apps/api/core";

export type Conversation = {
  id: number;
  householdId: string;
  title: string;
  archived: boolean;
};

export type ConversationMessage = {
  id: number;
  conversationId: number;
  author: "member" | "luna";
  body: string;
};

export type DocumentProcessingState =
  | "needsCloudConsent"
  | "inspectingWithAssistance"
  | "waitingForCloudAssistance"
  | "needsMemberDirection"
  | "possibleDuplicate"
  | "readyToFile"
  | "filing"
  | "cabinetUnavailable"
  | "filed"
  | "dismissed";

export type ConfidenceState = "confirmed" | "looksRight" | "needsChecking" | "unknown";

export type ReviewEvidence = {
  label: string;
  value: string;
};

export type ReviewField = {
  value: string | null;
  confidenceState: ConfidenceState;
};

export type DocumentContextReview = {
  documentType: ReviewField;
  serviceProvider: ReviewField;
  serviceProviderRelevance: ReviewField;
  addressee: ReviewField;
  property: ReviewField;
  propertyRelevance: ReviewField;
  account: ReviewField;
  amount: ReviewField;
  relevantDates: ReviewField[];
};

export type ClarificationQuestion = {
  field:
    | "documentType"
    | "serviceProvider"
    | "serviceProviderRelevance"
    | "addressee"
    | "property"
    | "propertyRelevance"
    | "account"
    | "amount"
    | "relevantDates";
  prompt: string;
};

export type FilingDecisionReview = {
  fileName: string;
  cabinetDestination: string;
  confirmed: boolean;
};

export type FilingRule = {
  id: number;
  documentType: string;
  serviceProvider: string;
  addressee: string;
  property: string | null;
  account: string | null;
  fileName: string;
  cabinetDestination: string;
};

export type FilingRuleSummary = FilingRule & {
  teacher: string;
  createdAt: string;
  paused: boolean;
  deleted: boolean;
  affectedDocuments: string[];
};

export type FilingRuleUpdate = Omit<FilingRule, "id">;

export type FilingRuleAuditEvent = {
  id: number;
  householdId: string;
  ruleId: number;
  kind: "updated" | "paused" | "resumed" | "deleted";
  subject: string;
  outcome: string;
};

export type FilingRuleReorganizationPreview = {
  ruleId: number;
  proposedDirectory: string;
  documents: Array<{
    arrivalId: number;
    originalName: string;
    currentDestination: string;
    proposedDestination: string;
  }>;
};

export type DuplicateKind = "exact" | "possible";

export type DuplicateDecision = "keepBoth" | "linkCopies" | "discardNew" | "updatedVersion";

export type DuplicateCandidate = {
  arrivalId: number;
  kind: DuplicateKind;
  originalName: string;
  checksum: string;
  filedDestination: string | null;
};

export type DuplicateReview = {
  candidates: DuplicateCandidate[];
};

export type DuplicateResolution = {
  decision: DuplicateDecision;
  relatedArrivalId: number;
  relatedOriginalName: string;
  duplicateKind?: DuplicateKind;
};

export type DuplicateAuditEvent = {
  id: number;
  householdId: string;
  kind: "duplicateDecisionRecorded" | "duplicatePreferenceApplied";
  decision: DuplicateDecision;
  subject: string;
  outcome: string;
  relatedArrivalId: number;
};

export type IntelligenceProviderDescriptor = {
  id: string;
  name: string;
  description: string;
  models: Array<{ id: string; name: string }>;
  managedByLuna: boolean;
  authUrl: string | null;
};

export type IntelligenceProviderStatus = {
  descriptor: IntelligenceProviderDescriptor;
  gatewayConfigured: boolean;
  configured: boolean;
};

export type IntelligenceSelection = {
  providerId: string;
  modelId: string;
};

export type IntelligenceResult = {
  requestId: string;
  documentArrivalId: string;
  providerId: string;
  modelId: string;
  consentGrantId: number;
  fields: Record<string, string>;
  evidence: Array<{ field: string; value: string; sourceReference: string | null }>;
  sourceReferences: string[];
  candidateDirection: {
    documentType: string | null;
    serviceProvider: string | null;
    addressee: string | null;
    property: string | null;
    account: string | null;
    amount: string | null;
    relevantDates: string[];
  } | null;
  usage: {
    inputTokens: number | null;
    outputTokens: number | null;
    estimatedCostUsd: number | null;
  };
};

export type CloudConsentDecision = "allowOnce" | "allowForScope" | "keepLocal" | "useExistingScope";

export type CloudConsentScope = {
  id: number;
  householdId: string;
  providerId: string;
  modelId: string;
  capability: "directionInterpretation";
  purpose: string;
  documentArrivalId: string | null;
  futureScope: string | null;
  fields: string[];
  kind: "oneTime" | "reusable";
  grantedBy: string;
  createdAt: string;
  consumedAt: string | null;
  revokedAt: string | null;
  revoked: boolean;
};

export type CloudAssistanceAuditEvent = {
  id: number;
  householdId: string;
  requestId: string;
  documentArrivalId: string;
  providerId: string;
  modelId: string;
  capability: "directionInterpretation";
  purpose: string;
  consent: CloudConsentDecision;
  consentGrantId: number | null;
  grantedBy: string;
  outcome: "completed" | "denied" | "waitingForRetry" | "cancelled";
  candidateDisposition: "pending" | "accepted" | "corrected" | "rejected";
  reason: string;
  usage: IntelligenceResult["usage"];
};

export type CloudAssistanceResolution = {
  result: IntelligenceResult | null;
  processingState: DocumentProcessingState;
};

export type ManualMoveCandidate = {
  arrivalId: number;
  originalName: string;
  previousDestination: string;
  currentDestination: string;
};

export type ContextRelevanceDirection = {
  subject: string;
  explanation: string;
};

export type DocumentContextDirection = {
  documentType: string | null;
  documentTypeResolved: boolean;
  serviceProvider: string | null;
  serviceProviderResolved: boolean;
  addressee: string | null;
  addresseeResolved: boolean;
  property: string | null;
  propertyResolved: boolean;
  account: string | null;
  accountResolved: boolean;
  amount: string | null;
  amountResolved: boolean;
  relevantDates: string[];
  relevantDatesResolved: boolean;
  serviceProviderRelevance: ContextRelevanceDirection | null;
  propertyRelevance: ContextRelevanceDirection | null;
};

export type FilingDecisionDirection = {
  fileName: string;
  cabinetDestination: string;
};

export type FiledOriginal = {
  arrivalId: number;
  conversationId: number;
  originalName: string;
  finalPath: string;
  checksum: string;
  sourcePath: string;
  filingDecision: FilingDecisionReview;
};

export type AuditEvent = {
  id: number;
  householdId: string;
  kind: "documentFiled" | "exactMatchHandledAutomatically";
  authority: "memberDirection" | "filingRule";
  subject: string;
  outcome: string;
  filedOriginal: FiledOriginal;
};

export type PortableHistoryEvent = {
  eventId: string;
  occurredAt: string;
  eventKind:
    | "documentFiled"
    | "exactMatchHandledAutomatically"
    | "filingRuleChanged"
    | "consentChanged"
    | "executionCompleted";
  authority: "memberDirection" | "filingRule" | "authorityGrant" | "consentGrant";
  subjectReference: string;
  outcome:
    | "filedAndVerified"
    | "filingRuleChanged"
    | "cloudAssistanceCompleted"
    | "keptLocal"
    | "waitingForCloudAssistance"
    | "cabinetUnavailable"
    | "providerUnavailable"
    | "failed";
  candidateDisposition?: "pending" | "accepted" | "corrected" | "rejected";
};

export type PortableTrustedDeviceAuthorization = {
  deviceId: string;
  authorizationPublicKey: string;
  activatedKeyEpoch: number;
  revokedAfter?: {
    keyEpoch: number;
    sequence: number;
    eventDigest: string;
  };
};

export type ReviewCard = {
  confidenceState: ConfidenceState;
  evidence: ReviewEvidence[];
  uncertainties: string[];
  proposedCabinetDestination: string | null;
  context: DocumentContextReview;
  questions: ClarificationQuestion[];
  filingDecision: FilingDecisionReview | null;
  learnedRule: FilingRule | null;
};

export type DocumentArrival = {
  id: number;
  householdId: string;
  conversationId: number;
  originalName: string;
  originalPath: string;
  sourcePath: string;
  checksum: string;
  mediaType: string;
  extractedText: string | null;
  reviewCard: ReviewCard;
  processingState: DocumentProcessingState;
  filedOriginal: FiledOriginal | null;
  duplicateReview: DuplicateReview | null;
  duplicateResolution: DuplicateResolution | null;
};

export type TodoItem = {
  arrivalId: number;
  conversationId: number;
  conversationTitle: string;
  conversationDeleted: boolean;
  documentName: string;
  processingState: DocumentProcessingState;
};

export interface ConversationService {
  createConversation(householdId: string, title: string): Promise<Conversation>;
  listConversations(householdId: string, search?: string, includeArchived?: boolean): Promise<Conversation[]>;
  renameConversation(householdId: string, conversationId: number, title: string): Promise<void>;
  archiveConversation(householdId: string, conversationId: number, archived: boolean): Promise<void>;
  deleteConversation(householdId: string, conversationId: number): Promise<void>;
  addMemberMessage(householdId: string, conversationId: number, body: string): Promise<ConversationMessage>;
  listMessages(householdId: string, conversationId: number): Promise<ConversationMessage[]>;
  selectDocumentFiles(): Promise<string[]>;
  attachDocument(householdId: string, conversationId: number, path: string): Promise<DocumentArrival>;
  resumeDocumentFilings(householdId: string): Promise<void>;
  listDocumentArrivals(householdId: string): Promise<DocumentArrival[]>;
  listTodoItems(householdId: string): Promise<TodoItem[]>;
  listFiledOriginals(householdId: string): Promise<FiledOriginal[]>;
  listAuditEvents(householdId: string): Promise<AuditEvent[]>;
  listPortableHistoryEvents(householdId: string): Promise<PortableHistoryEvent[]>;
  synchronizePortableMemory(
    householdId: string,
    trustedDevices: PortableTrustedDeviceAuthorization[],
  ): Promise<{ imported: number; duplicates: number }>;
  listDuplicateAuditEvents(householdId: string): Promise<DuplicateAuditEvent[]>;
  listCloudAssistanceAuditEvents(householdId: string): Promise<CloudAssistanceAuditEvent[]>;
  resolveDuplicate(
    householdId: string,
    arrivalId: number,
    relatedArrivalId: number,
    decision: DuplicateDecision,
    rememberPreference: boolean,
  ): Promise<DocumentArrival>;
  listFilingRules(householdId: string): Promise<FilingRuleSummary[]>;
  updateFilingRule(householdId: string, ruleId: number, update: FilingRuleUpdate): Promise<FilingRuleSummary>;
  pauseFilingRule(householdId: string, ruleId: number, paused: boolean): Promise<FilingRuleSummary>;
  deleteFilingRule(householdId: string, ruleId: number): Promise<FilingRuleSummary>;
  listFilingRuleAuditEvents(householdId: string): Promise<FilingRuleAuditEvent[]>;
  previewFilingRuleReorganization(householdId: string, ruleId: number, proposedDirectory: string): Promise<FilingRuleReorganizationPreview>;
  listManualMoveCandidates(householdId: string): Promise<ManualMoveCandidate[]>;
  recordManualMoveDecision(householdId: string, arrivalId: number, teachesRule: boolean): Promise<DocumentArrival>;
  dismissDocumentArrival(householdId: string, arrivalId: number): Promise<void>;
  recordMemberDirection(
    householdId: string,
    arrivalId: number,
    direction: DocumentContextDirection,
  ): Promise<DocumentArrival>;
  confirmFilingDecision(
    householdId: string,
    arrivalId: number,
    direction: FilingDecisionDirection,
  ): Promise<DocumentArrival>;
  listIntelligenceProviders(): Promise<IntelligenceProviderDescriptor[]>;
  listIntelligenceProviderStatuses(householdId: string): Promise<IntelligenceProviderStatus[]>;
  testAndSetIntelligenceProviderCredential(
    householdId: string,
    providerId: string,
    credential: string,
  ): Promise<void>;
  clearIntelligenceProviderCredential(householdId: string, providerId: string): Promise<void>;
  setManagedIntelligenceGatewayCredential(householdId: string, credential: string): Promise<void>;
  clearManagedIntelligenceGatewayCredential(householdId: string): Promise<void>;
  listCloudConsentScopes(householdId: string): Promise<CloudConsentScope[]>;
  revokeCloudConsentScope(householdId: string, scopeId: number): Promise<void>;
  evaluateDocumentWithCloudAssistance(
    householdId: string,
    arrivalId: number,
    selection: IntelligenceSelection,
    consent: CloudConsentDecision,
    existingConsentGrantId: number | null,
  ): Promise<CloudAssistanceResolution>;
  recordCloudCandidateDisposition(
    householdId: string,
    arrivalId: number,
    requestId: string,
    disposition: "accepted" | "corrected" | "rejected",
  ): Promise<void>;
}

export const tauriConversationService: ConversationService = {
  createConversation(householdId, title) {
    return invoke<Conversation>("create_conversation", { householdId, title });
  },
  listConversations(householdId, search, includeArchived = false) {
    return invoke<Conversation[]>("list_conversations", {
      householdId,
      search: search || null,
      includeArchived,
    });
  },
  renameConversation(householdId, conversationId, title) {
    return invoke("rename_conversation", { householdId, conversationId, title });
  },
  archiveConversation(householdId, conversationId, archived) {
    return invoke("archive_conversation", { householdId, conversationId, archived });
  },
  deleteConversation(householdId, conversationId) {
    return invoke("delete_conversation", { householdId, conversationId });
  },
  addMemberMessage(householdId, conversationId, body) {
    return invoke<ConversationMessage>("add_member_message", { householdId, conversationId, body });
  },
  listMessages(householdId, conversationId) {
    return invoke<ConversationMessage[]>("list_conversation_messages", { householdId, conversationId });
  },
  selectDocumentFiles() {
    return invoke<string[]>("select_document_files");
  },
  attachDocument(householdId, conversationId, path) {
    return invoke<DocumentArrival>("attach_document", { householdId, conversationId, path });
  },
  resumeDocumentFilings(householdId) {
    return invoke("resume_document_filings", { householdId });
  },
  listDocumentArrivals(householdId) {
    return invoke<DocumentArrival[]>("list_document_arrivals", { householdId });
  },
  listTodoItems(householdId) {
    return invoke<TodoItem[]>("list_todo_items", { householdId });
  },
  listFiledOriginals(householdId) {
    return invoke<FiledOriginal[]>("list_filed_originals", { householdId });
  },
  listAuditEvents(householdId) {
    return invoke<AuditEvent[]>("list_audit_events", { householdId });
  },
  listPortableHistoryEvents(householdId) {
    return invoke<PortableHistoryEvent[]>("list_portable_history_events", { householdId });
  },
  synchronizePortableMemory(householdId, trustedDevices) {
    return invoke("synchronize_portable_memory", { householdId, trustedDevices });
  },
  listDuplicateAuditEvents(householdId) {
    return invoke<DuplicateAuditEvent[]>("list_duplicate_audit_events", { householdId });
  },
  listCloudAssistanceAuditEvents(householdId) {
    return invoke<CloudAssistanceAuditEvent[]>("list_cloud_assistance_audit_events", { householdId });
  },
  resolveDuplicate(householdId, arrivalId, relatedArrivalId, decision, rememberPreference) {
    return invoke<DocumentArrival>("resolve_duplicate", {
      householdId,
      arrivalId,
      relatedArrivalId,
      decision,
      rememberPreference,
    });
  },
  listFilingRules(householdId) {
    return invoke<FilingRuleSummary[]>("list_filing_rules", { householdId });
  },
  updateFilingRule(householdId, ruleId, update) {
    return invoke<FilingRuleSummary>("update_filing_rule", { householdId, ruleId, update });
  },
  pauseFilingRule(householdId, ruleId, paused) {
    return invoke<FilingRuleSummary>("pause_filing_rule", { householdId, ruleId, paused });
  },
  deleteFilingRule(householdId, ruleId) {
    return invoke<FilingRuleSummary>("delete_filing_rule", { householdId, ruleId });
  },
  listFilingRuleAuditEvents(householdId) {
    return invoke<FilingRuleAuditEvent[]>("list_filing_rule_audit_events", { householdId });
  },
  previewFilingRuleReorganization(householdId, ruleId, proposedDirectory) {
    return invoke<FilingRuleReorganizationPreview>("preview_filing_rule_reorganization", {
      householdId,
      ruleId,
      proposedDirectory,
    });
  },
  listManualMoveCandidates(householdId) {
    return invoke<ManualMoveCandidate[]>("list_manual_move_candidates", { householdId });
  },
  recordManualMoveDecision(householdId, arrivalId, teachesRule) {
    return invoke<DocumentArrival>("record_manual_move_decision", {
      householdId,
      arrivalId,
      teachesRule,
    });
  },
  dismissDocumentArrival(householdId, arrivalId) {
    return invoke("dismiss_document_arrival", { householdId, arrivalId });
  },
  recordMemberDirection(householdId, arrivalId, direction) {
    return invoke<DocumentArrival>("record_member_direction", {
      householdId,
      arrivalId,
      direction,
    });
  },
  confirmFilingDecision(householdId, arrivalId, direction) {
    return invoke<DocumentArrival>("confirm_filing_decision", {
      householdId,
      arrivalId,
      direction,
    });
  },
  listIntelligenceProviders() {
    return invoke<IntelligenceProviderDescriptor[]>("list_intelligence_providers");
  },
  listIntelligenceProviderStatuses(householdId) {
    return invoke<IntelligenceProviderStatus[]>("list_intelligence_provider_statuses", { householdId });
  },
  testAndSetIntelligenceProviderCredential(householdId, providerId, credential) {
    return invoke("test_and_set_intelligence_provider_credential", {
      householdId,
      providerId,
      credential,
    });
  },
  clearIntelligenceProviderCredential(householdId, providerId) {
    return invoke("clear_intelligence_provider_credential", { householdId, providerId });
  },
  setManagedIntelligenceGatewayCredential(householdId, credential) {
    return invoke("set_managed_intelligence_gateway_credential", { householdId, credential });
  },
  clearManagedIntelligenceGatewayCredential(householdId) {
    return invoke("clear_managed_intelligence_gateway_credential", { householdId });
  },
  listCloudConsentScopes(householdId) {
    return invoke<CloudConsentScope[]>("list_cloud_consent_scopes", { householdId });
  },
  revokeCloudConsentScope(householdId, scopeId) {
    return invoke("revoke_cloud_consent_scope", { householdId, scopeId });
  },
  evaluateDocumentWithCloudAssistance(householdId, arrivalId, selection, consent, existingConsentGrantId) {
    return invoke<CloudAssistanceResolution>("evaluate_document_with_cloud_assistance", {
      input: {
        householdId,
        arrivalId,
        selection,
        consent,
        existingConsentGrantId,
      },
    });
  },
  recordCloudCandidateDisposition(householdId, arrivalId, requestId, disposition) {
    return invoke("record_cloud_candidate_disposition", {
      householdId,
      arrivalId,
      requestId,
      disposition,
    });
  },
};
