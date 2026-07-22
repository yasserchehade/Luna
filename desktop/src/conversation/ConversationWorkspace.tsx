import { FormEvent, useCallback, useEffect, useRef, useState } from "react";
import { getCurrentWebview } from "@tauri-apps/api/webview";
import type {
  Conversation,
  ConversationMessage,
  ConversationService,
  DocumentContextDirection,
  DocumentArrival,
  FilingDecisionDirection,
  TodoItem,
} from "./conversationService";

type ConversationWorkspaceProps = {
  conversationService: ConversationService;
  destination: "Luna" | "To do";
  householdId: string;
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
  readyToFile: "Ready to file",
  filing: "Filing",
  filed: "Filed",
  dismissed: "Dismissed",
})[arrival.processingState];

const confidenceLabel = (arrival: DocumentArrival) => ({
  confirmed: "Confirmed",
  looksRight: "Looks right",
  needsChecking: "Needs checking",
  unknown: "Unknown",
})[arrival.reviewCard.confidenceState];

type DocumentReviewEditorProps = {
  arrival: DocumentArrival;
  onConfirm(direction: FilingDecisionDirection): Promise<void>;
  onRecord(direction: DocumentContextDirection): Promise<void>;
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
  onConfirm,
  onRecord,
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
  const clarificationQuestions = arrival.processingState === "needsMemberDirection"
    ? arrival.reviewCard.questions.filter(({ field }) => field !== "amount")
    : arrival.reviewCard.questions;

  useEffect(() => {
    const refreshed = directionFromReview(arrival);
    setDirection(refreshed);
    setDatesDraft(refreshed.relevantDates.join(", "));
  }, [arrival, context]);

  useEffect(() => {
    setFileName(decision?.fileName ?? "");
    setCabinetDestination(decision?.cabinetDestination ?? "");
  }, [decision]);

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
          <div><small>{todo.conversationTitle}</small><h2>{todo.documentName}</h2><p>Needs your direction</p></div>
          <div>
            <button type="button" onClick={() => void openTodo(todo)}>Open Conversation item</button>
            <button type="button" onClick={() => void dismissArrival(todo.arrivalId)}>Dismiss</button>
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
              void conversationService.deleteConversation(householdId, selectedConversation.id)
                .then(() => loadHouseholdWork(selectedConversation.id));
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
          <DocumentReviewEditor
            arrival={arrival}
            onConfirm={(direction) => confirmDecision(arrival.id, direction)}
            onRecord={(direction) => recordDirection(arrival.id, direction)}
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
