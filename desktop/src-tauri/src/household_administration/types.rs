use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::{
    AvailableHouseholdTool, HouseholdActionProposal, HouseholdClarification, HouseholdContextItem,
    HouseholdWork,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HandleHouseholdAdministrationTurn {
    pub household_id: String,
    pub conversation_id: i64,
    pub member_message: String,
    pub source_reference: Option<String>,
    pub active_work_reference: Option<String>,
    pub authorised_household_context: Vec<HouseholdContextItem>,
    pub available_actions: Vec<AvailableHouseholdTool>,
    pub authorised_actor: String,
    pub request_id: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum HouseholdAdministrationFailureCategory {
    InvalidInput,
    SourceUnavailable,
    SourceTooLarge,
    UnsupportedSource,
    ReasoningUnavailable,
    MalformedProviderResult,
    IncompatibleContractVersion,
    MissingExistingWork,
    InvalidCorrectionTarget,
    PersistenceUnavailable,
}

#[derive(Debug, Clone, PartialEq, Eq, Error)]
#[error("{message}")]
pub struct HouseholdAdministrationFailure {
    pub category: HouseholdAdministrationFailureCategory,
    pub message: String,
}

impl HouseholdAdministrationFailure {
    pub(crate) fn new(
        category: HouseholdAdministrationFailureCategory,
        message: impl Into<String>,
    ) -> Self {
        Self {
            category,
            message: message.into(),
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct HouseholdAdministrationOutcome {
    pub request_id: String,
    pub message: String,
    pub work: Option<HouseholdWork>,
    pub clarification: Option<HouseholdClarification>,
    pub proposed_actions: Vec<HouseholdActionProposal>,
    pub audit_events: Vec<String>,
}
