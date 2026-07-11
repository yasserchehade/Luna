from enum import StrEnum

from pydantic import BaseModel, Field


class HouseholdEntity(BaseModel):
    id: str
    entity_type: str
    display_name: str
    metadata: dict[str, object] = Field(default_factory=dict)


class HouseholdGraphNode(BaseModel):
    id: str
    node_type: str
    display_name: str
    metadata: dict[str, object] = Field(default_factory=dict)


class HouseholdEntityCreate(BaseModel):
    entity_type: str = Field(min_length=1, max_length=80)
    display_name: str = Field(min_length=1, max_length=200)
    metadata: dict[str, object] = Field(default_factory=dict)


class HouseholdEntityUpdate(BaseModel):
    entity_type: str | None = Field(default=None, min_length=1, max_length=80)
    display_name: str | None = Field(default=None, min_length=1, max_length=200)
    metadata: dict[str, object] | None = None


class HouseholdEntityActionResponse(BaseModel):
    entity: HouseholdEntity


class EntityRelationship(BaseModel):
    id: str
    source_entity_type: str
    source_entity_id: str
    relationship_type: str
    target_entity_type: str
    target_entity_id: str
    provenance_document_id: str | None = None
    confidence: float | None = None


class EntityRelationshipCreate(BaseModel):
    source_entity_id: str
    relationship_type: str = Field(min_length=1, max_length=80)
    target_entity_id: str
    source_entity_type: str | None = Field(default=None, min_length=1, max_length=80)
    target_entity_type: str | None = Field(default=None, min_length=1, max_length=80)
    provenance_document_id: str | None = None
    confidence: float | None = None


class EntityRelationshipUpdate(BaseModel):
    source_entity_id: str | None = None
    relationship_type: str | None = Field(default=None, min_length=1, max_length=80)
    target_entity_id: str | None = None
    source_entity_type: str | None = Field(default=None, min_length=1, max_length=80)
    target_entity_type: str | None = Field(default=None, min_length=1, max_length=80)
    provenance_document_id: str | None = None
    confidence: float | None = None


class EntityRelationshipActionResponse(BaseModel):
    relationship: EntityRelationship


class HouseholdEntityDeleteResponse(BaseModel):
    deleted_entity_id: str


class EntityRelationshipDeleteResponse(BaseModel):
    deleted_relationship_id: str


class EntityRelationshipsForEntity(BaseModel):
    entity_id: str
    relationships: list[EntityRelationship]


class HouseholdGraph(BaseModel):
    nodes: list[HouseholdGraphNode]
    relationships: list[EntityRelationship]


class GraphSuggestionStatus(StrEnum):
    pending = "pending"
    accepted = "accepted"
    rejected = "rejected"


class GraphSuggestionAction(StrEnum):
    create_entity = "create_entity"
    connect_entities = "connect_entities"
    update_metadata = "update_metadata"
    attach_document = "attach_document"
    merge_duplicate_entities = "merge_duplicate_entities"


class GraphSuggestion(BaseModel):
    id: str
    confidence: float
    suggested_action: GraphSuggestionAction
    reasoning: str
    affected_entities: list[dict[str, object]] = Field(default_factory=list)
    status: GraphSuggestionStatus
    action_payload: dict[str, object] = Field(default_factory=dict)
    source_document_id: str | None = None
    source_bill_id: str | None = None


class GraphSuggestionList(BaseModel):
    suggestions: list[GraphSuggestion]


class GraphSuggestionActionResponse(BaseModel):
    suggestion: GraphSuggestion


class TaskStatus(StrEnum):
    open = "open"
    done = "done"
    dismissed = "dismissed"
    archived = "archived"


class Task(BaseModel):
    id: str
    title: str
    description: str | None = None
    status: TaskStatus
    due_date: str | None = None
    related_entity_type: str | None = None
    related_entity_id: str | None = None


class TaskCreate(BaseModel):
    title: str = Field(min_length=1, max_length=240)
    description: str | None = None
    due_date: str | None = None
    related_entity_type: str | None = None
    related_entity_id: str | None = None


class ReminderStatus(StrEnum):
    scheduled = "scheduled"
    sent = "sent"
    dismissed = "dismissed"
    archived = "archived"


class Reminder(BaseModel):
    id: str
    title: str
    remind_at: str
    status: ReminderStatus
    related_entity_type: str | None = None
    related_entity_id: str | None = None


class ReminderCreate(BaseModel):
    title: str = Field(min_length=1, max_length=240)
    remind_at: str
    related_entity_type: str | None = None
    related_entity_id: str | None = None


class ObligationStatus(StrEnum):
    needs_review = "needs_review"
    upcoming = "upcoming"
    due_soon = "due_soon"
    overdue = "overdue"
    paid = "paid"
    archived = "archived"


class Obligation(BaseModel):
    id: str
    source_bill_id: str | None = None
    title: str
    supplier: str | None = None
    amount: float | None = None
    currency: str = "AUD"
    due_date: str | None = None
    status: ObligationStatus
    evidence: dict[str, object] = Field(default_factory=dict)


class HouseholdSummary(BaseModel):
    entities: list[HouseholdEntity]
    open_tasks: list[Task]
    upcoming_reminders: list[Reminder]
    upcoming_obligations: list[Obligation] = Field(default_factory=list)
    overdue_obligations: list[Obligation] = Field(default_factory=list)
    needs_review_obligations: list[Obligation] = Field(default_factory=list)


class TaskActionResponse(BaseModel):
    task: Task


class ReminderActionResponse(BaseModel):
    reminder: Reminder
