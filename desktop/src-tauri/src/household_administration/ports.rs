use std::time::{SystemTime, UNIX_EPOCH};

use thiserror::Error;

use crate::{
    HouseholdAdministrationMessage, HouseholdAdministrationRequest, HouseholdAdministrationSource,
    HouseholdWork, UntrustedHouseholdAdministrationResult,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
pub enum ConversationPortError {
    #[error("conversation persistence is unavailable")]
    Unavailable,
}

pub trait ConversationPort: Send + Sync {
    fn recent_messages(
        &self,
        household_id: &str,
        conversation_id: i64,
        limit: usize,
    ) -> Result<Vec<HouseholdAdministrationMessage>, ConversationPortError>;

    fn append_member_message(
        &self,
        household_id: &str,
        conversation_id: i64,
        body: &str,
    ) -> Result<(), ConversationPortError>;

    fn append_luna_message(
        &self,
        household_id: &str,
        conversation_id: i64,
        body: &str,
        source_reference: Option<&str>,
    ) -> Result<(), ConversationPortError>;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
pub enum HouseholdWorkPortError {
    #[error("Household Work persistence is unavailable")]
    Unavailable,
}

pub trait HouseholdWorkPort: Send + Sync {
    fn list(&self, household_id: &str) -> Result<Vec<HouseholdWork>, HouseholdWorkPortError>;

    fn save(&self, household_id: &str, work: &HouseholdWork) -> Result<(), HouseholdWorkPortError>;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
pub enum SourcePortError {
    #[error("the source is unavailable")]
    Unavailable,
    #[error("the source does not exist")]
    NotFound,
    #[error("the source exceeds the Household Administration processing limit")]
    TooLarge,
    #[error("the source type is unsupported")]
    Unsupported,
}

pub trait SourcePort: Send + Sync {
    fn load(
        &self,
        household_id: &str,
        source_reference: &str,
    ) -> Result<HouseholdAdministrationSource, SourcePortError>;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
pub enum ReasoningPortError {
    #[error("the Household Administration reasoning route is unavailable")]
    Unavailable,
    #[error("the Household Administration result is malformed")]
    MalformedResult,
    #[error("the Household Administration contract version is incompatible")]
    IncompatibleContractVersion,
}

pub trait HouseholdAdministrationReasoning: Send + Sync {
    fn reason(
        &self,
        request: &HouseholdAdministrationRequest,
    ) -> Result<UntrustedHouseholdAdministrationResult, ReasoningPortError>;
}

pub trait HouseholdAdministrationClock: Send + Sync {
    fn now(&self) -> String;
}

#[derive(Debug, Clone)]
pub struct FixedHouseholdAdministrationClock {
    now: String,
}

impl FixedHouseholdAdministrationClock {
    pub fn new(now: impl Into<String>) -> Self {
        Self { now: now.into() }
    }
}

impl HouseholdAdministrationClock for FixedHouseholdAdministrationClock {
    fn now(&self) -> String {
        self.now.clone()
    }
}

#[derive(Debug, Clone, Copy, Default)]
pub struct SystemHouseholdAdministrationClock;

impl HouseholdAdministrationClock for SystemHouseholdAdministrationClock {
    fn now(&self) -> String {
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|duration| format!("{}-{}", duration.as_secs(), duration.subsec_nanos()))
            .unwrap_or_else(|_| "0-0".to_owned())
    }
}
