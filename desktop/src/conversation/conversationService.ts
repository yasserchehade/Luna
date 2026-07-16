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

export type DocumentProcessingState = "needsMemberDirection" | "dismissed";

export type ConfidenceState = "confirmed" | "looksRight" | "needsChecking" | "unknown";

export type ReviewEvidence = {
  label: string;
  value: string;
};

export type ReviewCard = {
  confidenceState: ConfidenceState;
  evidence: ReviewEvidence[];
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
};

export type TodoItem = {
  arrivalId: number;
  conversationId: number;
  conversationTitle: string;
  conversationDeleted: boolean;
  documentName: string;
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
  listDocumentArrivals(householdId: string): Promise<DocumentArrival[]>;
  listTodoItems(householdId: string): Promise<TodoItem[]>;
  dismissDocumentArrival(householdId: string, arrivalId: number): Promise<void>;
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
  listDocumentArrivals(householdId) {
    return invoke<DocumentArrival[]>("list_document_arrivals", { householdId });
  },
  listTodoItems(householdId) {
    return invoke<TodoItem[]>("list_todo_items", { householdId });
  },
  dismissDocumentArrival(householdId, arrivalId) {
    return invoke("dismiss_document_arrival", { householdId, arrivalId });
  },
};
