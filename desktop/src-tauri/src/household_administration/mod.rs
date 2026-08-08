mod engine;
mod openai;
mod ports;
mod types;

pub(crate) use engine::apply_result;
pub use engine::HouseholdAdministrationEngine;
pub(crate) use openai::household_administration_response_schema;
pub use openai::{
    OpenAiHouseholdAdministrationReasoningAdapter, HOUSEHOLD_ADMINISTRATION_CONTRACT_VERSION,
};
pub use ports::{
    ConversationPort, ConversationPortError, FixedHouseholdAdministrationClock,
    HouseholdAdministrationClock, HouseholdAdministrationReasoning, HouseholdWorkPort,
    HouseholdWorkPortError, ReasoningPortError, SourcePort, SourcePortError,
    SystemHouseholdAdministrationClock,
};
pub use types::{
    HandleHouseholdAdministrationTurn, HandleHouseholdWorkCommand, HouseholdAdministrationFailure,
    HouseholdAdministrationFailureCategory, HouseholdAdministrationOutcome, HouseholdWorkCommand,
};
