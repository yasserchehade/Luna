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

export type DocumentProcessingState = "needsMemberDirection" | "possibleDuplicate" | "readyToFile" | "filing" | "filed" | "dismissed";

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
  listDuplicateAuditEvents(householdId: string): Promise<DuplicateAuditEvent[]>;
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
  listDuplicateAuditEvents(householdId) {
    return invoke<DuplicateAuditEvent[]>("list_duplicate_audit_events", { householdId });
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
};
