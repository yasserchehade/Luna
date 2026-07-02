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
    provenance_document_id: str | None = None
    confidence: float | None = None


class EntityRelationshipActionResponse(BaseModel):
    relationship: EntityRelationship


class HouseholdGraph(BaseModel):
    nodes: list[HouseholdGraphNode]
    relationships: list[EntityRelationship]


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


class HouseholdSummary(BaseModel):
    entities: list[HouseholdEntity]
    open_tasks: list[Task]
    upcoming_reminders: list[Reminder]


class TaskActionResponse(BaseModel):
    task: Task


class ReminderActionResponse(BaseModel):
    reminder: Reminder
