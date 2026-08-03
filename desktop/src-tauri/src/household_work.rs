use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum HouseholdWorkStatus {
    Active,
    NeedsClarification,
    AwaitingApproval,
    InProgress,
    Monitoring,
    Completed,
    Dismissed,
    NoLongerRelevant,
    Blocked,
}

impl HouseholdWorkStatus {
    pub fn is_terminal(self) -> bool {
        matches!(
            self,
            Self::Completed | Self::Dismissed | Self::NoLongerRelevant
        )
    }

    pub fn is_open(self) -> bool {
        !self.is_terminal()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ValidatedHouseholdWorkDirection {
    Complete,
    Dismiss,
    MarkNoLongerRelevant,
    Reopen,
}

impl ValidatedHouseholdWorkDirection {
    pub fn terminal_status(self) -> Option<HouseholdWorkStatus> {
        match self {
            Self::Complete => Some(HouseholdWorkStatus::Completed),
            Self::Dismiss => Some(HouseholdWorkStatus::Dismissed),
            Self::MarkNoLongerRelevant => Some(HouseholdWorkStatus::NoLongerRelevant),
            Self::Reopen => None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum HouseholdWorkKind {
    Bill,
    Renewal,
    Request,
    Appointment,
    Other,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum WorkFactKey {
    Provider,
    Property,
    Account,
    Amount,
    DueDate,
    RequiredAction,
    Urgency,
    Other,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum WorkFactCertainty {
    Confirmed,
    Likely,
    Unknown,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ProposedActionKind {
    DraftReply,
    Reminder,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ActionApproval {
    NotRequired,
    Required,
    Approved,
    Declined,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ActionExecution {
    NotStarted,
    Completed,
    Failed,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[serde(deny_unknown_fields)]
pub struct WorkFact {
    pub key: WorkFactKey,
    pub value: String,
    pub evidence_refs: Vec<String>,
    pub certainty: WorkFactCertainty,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProposedAction {
    pub id: String,
    pub kind: ProposedActionKind,
    pub summary: String,
    pub arguments: BTreeMap<String, String>,
    pub approval: ActionApproval,
    pub execution: ActionExecution,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HouseholdWork {
    pub id: String,
    pub household_id: String,
    pub status: HouseholdWorkStatus,
    pub kind: HouseholdWorkKind,
    pub summary: String,
    pub facts: Vec<WorkFact>,
    pub source_refs: Vec<String>,
    pub responsible_member_id: Option<String>,
    pub due_at: Option<String>,
    pub urgency: Option<String>,
    pub proposed_actions: Vec<ProposedAction>,
    pub audit_events: Vec<String>,
    pub created_at: String,
    pub updated_at: String,
    pub closed_at: Option<String>,
}

impl HouseholdWork {
    pub fn new(
        id: impl Into<String>,
        household_id: impl Into<String>,
        kind: HouseholdWorkKind,
        summary: impl Into<String>,
        now: impl Into<String>,
    ) -> Self {
        let now = now.into();
        Self {
            id: id.into(),
            household_id: household_id.into(),
            status: HouseholdWorkStatus::Active,
            kind,
            summary: summary.into(),
            facts: Vec::new(),
            source_refs: Vec::new(),
            responsible_member_id: None,
            due_at: None,
            urgency: None,
            proposed_actions: Vec::new(),
            audit_events: Vec::new(),
            created_at: now.clone(),
            updated_at: now,
            closed_at: None,
        }
    }

    pub fn record_audit(&mut self, event: impl Into<String>, now: impl Into<String>) {
        self.audit_events.push(event.into());
        self.updated_at = now.into();
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HouseholdWorkSummary {
    pub id: String,
    pub status: HouseholdWorkStatus,
    pub kind: HouseholdWorkKind,
    pub summary: String,
    pub facts: Vec<WorkFact>,
    pub source_refs: Vec<String>,
    pub due_at: Option<String>,
    pub urgency: Option<String>,
}

impl From<&HouseholdWork> for HouseholdWorkSummary {
    fn from(work: &HouseholdWork) -> Self {
        Self {
            id: work.id.clone(),
            status: work.status,
            kind: work.kind,
            summary: work.summary.clone(),
            facts: work.facts.clone(),
            source_refs: work.source_refs.clone(),
            due_at: work.due_at.clone(),
            urgency: work.urgency.clone(),
        }
    }
}
