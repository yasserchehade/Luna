from enum import StrEnum

from pydantic import BaseModel, Field


class Capability(StrEnum):
    read = "read"
    write = "write"
    execute = "execute"


class AuthorityDecision(StrEnum):
    allowed = "allowed"
    approval_required = "approval_required"
    blocked = "blocked"
    escalate = "escalate"


class HouseholdRole(StrEnum):
    owner = "owner"
    admin = "admin"
    member = "member"
    viewer = "viewer"


class WorkOrderStatus(StrEnum):
    observed = "observed"
    prepared = "prepared"
    proposed = "proposed"
    approval_requested = "approval_requested"
    approved = "approved"
    executed = "executed"
    escalated = "escalated"
    rejected = "rejected"
    dismissed = "dismissed"


class ApprovalRequestStatus(StrEnum):
    pending = "pending"
    approved = "approved"
    rejected = "rejected"
    dismissed = "dismissed"
    escalated = "escalated"


class ConnectionScopeStatus(StrEnum):
    planned = "planned"
    connected = "connected"
    disabled = "disabled"
    revoked = "revoked"
    error = "error"


class AuthorityPolicy(BaseModel):
    id: str
    name: str
    work_type: str
    capability: Capability
    decision: AuthorityDecision
    approver_role: HouseholdRole | None = None
    spending_limit: float | None = None
    escalation_rule: str | None = None
    enabled: bool = True
    metadata: dict[str, object] = Field(default_factory=dict)


class AuthorityPolicyCreate(BaseModel):
    name: str = Field(min_length=1, max_length=200)
    work_type: str = Field(min_length=1, max_length=120)
    capability: Capability
    decision: AuthorityDecision = AuthorityDecision.approval_required
    approver_role: HouseholdRole | None = HouseholdRole.owner
    spending_limit: float | None = None
    escalation_rule: str | None = None
    metadata: dict[str, object] = Field(default_factory=dict)


class ConnectionScope(BaseModel):
    id: str
    provider: str
    connection_name: str
    capability: Capability
    granted_scopes: list[str] = Field(default_factory=list)
    status: ConnectionScopeStatus
    metadata: dict[str, object] = Field(default_factory=dict)


class WorkOrder(BaseModel):
    id: str
    work_type: str
    title: str
    description: str | None = None
    status: WorkOrderStatus
    capability_required: Capability
    subject_entity_type: str | None = None
    subject_entity_id: str | None = None
    source_document_id: str | None = None
    source_bill_id: str | None = None
    evidence: dict[str, object] = Field(default_factory=dict)
    result: dict[str, object] = Field(default_factory=dict)


class WorkOrderCreate(BaseModel):
    work_type: str = Field(min_length=1, max_length=120)
    title: str = Field(min_length=1, max_length=240)
    description: str | None = None
    status: WorkOrderStatus = WorkOrderStatus.prepared
    capability_required: Capability = Capability.write
    subject_entity_type: str | None = None
    subject_entity_id: str | None = None
    source_document_id: str | None = None
    source_bill_id: str | None = None
    evidence: dict[str, object] = Field(default_factory=dict)


class ApprovalRequest(BaseModel):
    id: str
    work_order_id: str
    status: ApprovalRequestStatus
    requested_approver_role: HouseholdRole | None = None
    reason: str
    decision_reason: str | None = None


class WorkOrderWithApproval(BaseModel):
    work_order: WorkOrder
    approval_request: ApprovalRequest | None = None


class ApprovalDecisionRequest(BaseModel):
    reason: str | None = None


class ApprovalDecisionResponse(BaseModel):
    approval_request: ApprovalRequest
    work_order: WorkOrder
