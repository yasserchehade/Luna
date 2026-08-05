mod engine;
mod ports;
mod types;

pub(crate) use engine::apply_result;
pub use engine::HouseholdAdministrationEngine;
pub use ports::{
    ConversationPort, ConversationPortError, FixedHouseholdAdministrationClock,
    HouseholdAdministrationClock, HouseholdAdministrationReasoning, HouseholdWorkPort,
    HouseholdWorkPortError, ReasoningPortError, SourcePort, SourcePortError,
    SystemHouseholdAdministrationClock,
};
pub use types::{
    HandleHouseholdAdministrationTurn, HouseholdAdministrationFailure,
    HouseholdAdministrationFailureCategory, HouseholdAdministrationOutcome,
};
