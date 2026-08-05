use crate::{
    intelligence::validate_household_result, ActionApproval, ActionExecution,
    HouseholdAdministrationRequest, HouseholdWork, HouseholdWorkKind, HouseholdWorkOperation,
    HouseholdWorkStatus, HouseholdWorkSummary, IntelligenceExecutionConstraints,
    IntelligenceFailure, ProposedAction, ValidatedHouseholdWorkDirection,
    MAX_HOUSEHOLD_EXTRACTED_TEXT_CHARS, MAX_MVP_DOCUMENT_BYTES,
};

use super::{
    ConversationPort, HandleHouseholdAdministrationTurn, HouseholdAdministrationClock,
    HouseholdAdministrationFailure, HouseholdAdministrationFailureCategory,
    HouseholdAdministrationOutcome, HouseholdAdministrationReasoning, HouseholdWorkPort,
    ReasoningPortError, SourcePort, SourcePortError, HOUSEHOLD_ADMINISTRATION_CONTRACT_VERSION,
};

const RECENT_CONVERSATION_LIMIT: usize = 12;

pub struct HouseholdAdministrationEngine<'a> {
    conversations: &'a dyn ConversationPort,
    household_work: &'a dyn HouseholdWorkPort,
    sources: &'a dyn SourcePort,
    reasoning: &'a dyn HouseholdAdministrationReasoning,
    clock: &'a dyn HouseholdAdministrationClock,
}

impl<'a> HouseholdAdministrationEngine<'a> {
    pub fn new(
        conversations: &'a dyn ConversationPort,
        household_work: &'a dyn HouseholdWorkPort,
        sources: &'a dyn SourcePort,
        reasoning: &'a dyn HouseholdAdministrationReasoning,
        clock: &'a dyn HouseholdAdministrationClock,
    ) -> Self {
        Self {
            conversations,
            household_work,
            sources,
            reasoning,
            clock,
        }
    }

    pub fn handle_turn(
        &self,
        input: HandleHouseholdAdministrationTurn,
    ) -> Result<HouseholdAdministrationOutcome, HouseholdAdministrationFailure> {
        let member_message = input.member_message.trim();
        if input.household_id.trim().is_empty()
            || input.conversation_id <= 0
            || member_message.is_empty()
            || input.authorised_actor.trim().is_empty()
            || input.request_id.trim().is_empty()
        {
            return Err(failure(
                HouseholdAdministrationFailureCategory::InvalidInput,
                "The Household Administration turn is incomplete.",
            ));
        }

        self.conversations
            .append_member_message(&input.household_id, input.conversation_id, member_message)
            .map_err(|_| persistence_failure())?;
        let relevant_conversation = self
            .conversations
            .recent_messages(
                &input.household_id,
                input.conversation_id,
                RECENT_CONVERSATION_LIMIT,
            )
            .map_err(|_| persistence_failure())?;
        let source = input
            .source_reference
            .as_deref()
            .map(|reference| self.sources.load(&input.household_id, reference))
            .transpose()
            .map_err(source_failure)?;
        if let Some(source) = source.as_ref() {
            if source.original_size_bytes > MAX_MVP_DOCUMENT_BYTES
                || source
                    .extracted_text
                    .as_ref()
                    .is_some_and(|text| text.chars().count() > MAX_HOUSEHOLD_EXTRACTED_TEXT_CHARS)
            {
                return Err(failure(
                    HouseholdAdministrationFailureCategory::SourceTooLarge,
                    "The source exceeds Luna's Household Administration processing limit.",
                ));
            }
            if !matches!(
                source.media_type.as_str(),
                "application/pdf" | "image/jpeg" | "image/png"
            ) {
                return Err(failure(
                    HouseholdAdministrationFailureCategory::UnsupportedSource,
                    "Luna cannot process this source type yet.",
                ));
            }
        }

        let all_work = self
            .household_work
            .list(&input.household_id)
            .map_err(|_| persistence_failure())?;
        let explicit_target = input
            .active_work_reference
            .as_deref()
            .map(|work_id| {
                all_work
                    .iter()
                    .find(|work| work.id == work_id)
                    .ok_or_else(|| missing_target_failure(member_message))
            })
            .transpose()?;
        let source_linked = explicit_target.or_else(|| {
            input.source_reference.as_ref().and_then(|reference| {
                all_work
                    .iter()
                    .find(|work| work.source_refs.iter().any(|source| source == reference))
            })
        });
        if explicit_target.is_none()
            && source_linked.is_none()
            && is_correction(member_message)
            && all_work.iter().filter(|work| work.status.is_open()).count() > 1
        {
            return Err(failure(
                HouseholdAdministrationFailureCategory::InvalidCorrectionTarget,
                "Luna cannot safely determine which Household Work the correction targets.",
            ));
        }
        let active_household_work = all_work
            .iter()
            .filter(|work| work.status.is_open())
            .map(HouseholdWorkSummary::from)
            .collect();
        let source_linked_household_work = source_linked.map(HouseholdWorkSummary::from);
        let reasoning_request = HouseholdAdministrationRequest {
            request_id: input.request_id.clone(),
            conversation_id: input.conversation_id,
            current_message: member_message.to_owned(),
            relevant_conversation,
            source,
            household_context: input.authorised_household_context,
            active_household_work,
            source_linked_household_work,
            available_tools: input.available_actions,
            authority_and_approval_constraints: format!(
                "Luna validates all proposals. The member {} remains the authority. OpenAI cannot approve, execute, send, schedule, mutate context or close work.",
                input.authorised_actor
            ),
            response_schema_version: HOUSEHOLD_ADMINISTRATION_CONTRACT_VERSION.to_owned(),
            constraints: IntelligenceExecutionConstraints {
                timeout_ms: 30_000,
                max_output_tokens: 1_200,
            },
        };
        let untrusted = self
            .reasoning
            .reason(&reasoning_request)
            .map_err(reasoning_failure)?;
        if matches!(untrusted.work.operation, HouseholdWorkOperation::Update) {
            let Some(work_id) = untrusted.work.work_id.as_deref() else {
                return Err(failure(
                    HouseholdAdministrationFailureCategory::MalformedProviderResult,
                    "OpenAI returned an invalid Household Administration result.",
                ));
            };
            let valid_target = reasoning_request
                .active_household_work
                .iter()
                .any(|work| work.id == work_id)
                || reasoning_request
                    .source_linked_household_work
                    .as_ref()
                    .is_some_and(|work| work.id == work_id);
            if !valid_target {
                return Err(failure(
                    HouseholdAdministrationFailureCategory::MissingExistingWork,
                    "The Household Work selected for this update no longer exists.",
                ));
            }
        }
        let validated = validate_household_result(&reasoning_request, untrusted).map_err(
            |error| match error {
                IntelligenceFailure::UnsupportedCapability => failure(
                    HouseholdAdministrationFailureCategory::IncompatibleContractVersion,
                    "The Household Administration contract version is incompatible.",
                ),
                _ => failure(
                    HouseholdAdministrationFailureCategory::MalformedProviderResult,
                    "OpenAI returned an invalid Household Administration result.",
                ),
            },
        )?;
        let now = self.clock.now();
        let existing = validated
            .work
            .work_id
            .as_deref()
            .and_then(|id| all_work.iter().find(|work| work.id == id))
            .or(source_linked)
            .cloned();
        let work = apply_result(
            &input.household_id,
            input.conversation_id,
            input.source_reference.as_deref(),
            existing,
            &validated,
            &now,
        )?;
        if !matches!(validated.work.operation, HouseholdWorkOperation::None) {
            if let Some(work) = work.as_ref() {
                self.household_work
                    .save(&input.household_id, work)
                    .map_err(|_| persistence_failure())?;
            }
        }
        let mut message = validated.reply.clone();
        if let Some(clarification) = validated.clarification.as_ref() {
            message.push_str("\n\n");
            message.push_str(&clarification.question);
        }
        self.conversations
            .append_luna_message(
                &input.household_id,
                input.conversation_id,
                &message,
                input.source_reference.as_deref(),
            )
            .map_err(|_| persistence_failure())?;
        let audit_events = work
            .as_ref()
            .map(|work| work.audit_events.clone())
            .unwrap_or_default();
        Ok(HouseholdAdministrationOutcome {
            request_id: input.request_id,
            message,
            work,
            clarification: validated.clarification,
            proposed_actions: validated.proposed_actions,
            audit_events,
        })
    }
}

pub(crate) fn apply_result(
    household_id: &str,
    conversation_id: i64,
    source_reference: Option<&str>,
    existing: Option<HouseholdWork>,
    result: &crate::HouseholdAdministrationResult,
    now: &str,
) -> Result<Option<HouseholdWork>, HouseholdAdministrationFailure> {
    if matches!(result.work.operation, HouseholdWorkOperation::None) {
        return Ok(existing);
    }
    if matches!(result.work.operation, HouseholdWorkOperation::Update) && existing.is_none() {
        return Err(failure(
            HouseholdAdministrationFailureCategory::MissingExistingWork,
            "The Household Work selected for this update no longer exists.",
        ));
    }
    let existing_status = existing.as_ref().map(|work| work.status);
    if let Some(status) = result.work.status.filter(|status| status.is_terminal()) {
        if !matches!(result.work.operation, HouseholdWorkOperation::Update)
            || result
                .validated_member_direction
                .and_then(ValidatedHouseholdWorkDirection::terminal_status)
                != Some(status)
        {
            return Err(failure(
                HouseholdAdministrationFailureCategory::MalformedProviderResult,
                "OpenAI returned an unauthorised terminal Household Work transition.",
            ));
        }
    }
    if existing_status.is_some_and(HouseholdWorkStatus::is_terminal)
        && (result.validated_member_direction != Some(ValidatedHouseholdWorkDirection::Reopen)
            || result
                .work
                .status
                .is_some_and(HouseholdWorkStatus::is_terminal))
    {
        return Err(failure(
            HouseholdAdministrationFailureCategory::InvalidCorrectionTarget,
            "The terminal Household Work cannot be changed without validated corrective direction.",
        ));
    }
    let mut work = existing.unwrap_or_else(|| {
        let source_id = source_reference
            .and_then(|reference| reference.strip_prefix("document-"))
            .or(source_reference)
            .unwrap_or("0");
        HouseholdWork::new(
            format!("work-{now}-{conversation_id}-{source_id}"),
            household_id,
            result.work.kind.unwrap_or(HouseholdWorkKind::Other),
            result
                .work
                .summary
                .clone()
                .unwrap_or_else(|| "Household work requires attention.".to_owned()),
            now,
        )
    });
    if let Some(kind) = result.work.kind {
        work.kind = kind;
    }
    if let Some(summary) = result
        .work
        .summary
        .as_ref()
        .filter(|value| !value.trim().is_empty())
    {
        work.summary = summary.trim().to_owned();
    }
    for fact in &result.work.facts {
        work.facts.retain(|existing| existing.key != fact.key);
        work.facts.push(fact.clone());
    }
    if let Some(source_reference) = source_reference {
        if !work
            .source_refs
            .iter()
            .any(|source| source == source_reference)
        {
            work.source_refs.push(source_reference.to_owned());
        }
    }
    let conversation_reference = format!("conversation-{conversation_id}");
    if !work.source_refs.contains(&conversation_reference) {
        work.source_refs.push(conversation_reference);
    }
    work.due_at = result.work.due_at.clone().or(work.due_at);
    work.urgency = result.work.urgency.clone().or(work.urgency);
    if let Some(status) = result.work.status {
        work.status = status;
    } else if result.clarification.is_some() {
        work.status = HouseholdWorkStatus::NeedsClarification;
    } else if result
        .proposed_actions
        .iter()
        .any(|action| action.approval_required)
    {
        work.status = HouseholdWorkStatus::AwaitingApproval;
    } else {
        work.status = HouseholdWorkStatus::Active;
    }
    if !result.proposed_actions.is_empty() {
        work.proposed_actions = result
            .proposed_actions
            .iter()
            .enumerate()
            .map(|(index, action)| ProposedAction {
                id: format!("{}-action-{}", work.id, index + 1),
                kind: action.kind,
                summary: action.summary.trim().to_owned(),
                arguments: action.arguments.clone(),
                approval: if action.approval_required {
                    ActionApproval::Required
                } else {
                    ActionApproval::NotRequired
                },
                execution: ActionExecution::NotStarted,
            })
            .collect();
    }
    if work.status.is_terminal() {
        work.closed_at = Some(now.to_owned());
    } else {
        work.closed_at = None;
    }
    let event = match result.work.operation {
        HouseholdWorkOperation::Create => "Household Work created",
        HouseholdWorkOperation::Update => "Household Work updated",
        HouseholdWorkOperation::None => unreachable!("no-op results return before mutation"),
    };
    work.record_audit(event, now);
    if let Some(clarification) = result.clarification.as_ref() {
        work.record_audit(
            format!("Clarification requested: {}", clarification.question.trim()),
            now,
        );
    }
    Ok(Some(work))
}

fn reasoning_failure(error: ReasoningPortError) -> HouseholdAdministrationFailure {
    match error {
        ReasoningPortError::MissingApiKey => failure(
            HouseholdAdministrationFailureCategory::MissingApiKey,
            "Luna's OpenAI reasoning service is not configured.",
        ),
        ReasoningPortError::InvalidApiKey => failure(
            HouseholdAdministrationFailureCategory::InvalidApiKey,
            "Luna could not authenticate with OpenAI.",
        ),
        ReasoningPortError::ModelUnavailable => failure(
            HouseholdAdministrationFailureCategory::ModelUnavailable,
            "Luna's configured OpenAI model is unavailable.",
        ),
        ReasoningPortError::RateLimited => failure(
            HouseholdAdministrationFailureCategory::RateLimited,
            "OpenAI is temporarily rate limiting Luna. Please try again shortly.",
        ),
        ReasoningPortError::RequestTooLarge => failure(
            HouseholdAdministrationFailureCategory::RequestTooLarge,
            "The source is too large for Luna's OpenAI processing limit.",
        ),
        ReasoningPortError::UnsupportedMedia => failure(
            HouseholdAdministrationFailureCategory::UnsupportedMedia,
            "Luna cannot send this source type to OpenAI.",
        ),
        ReasoningPortError::NetworkFailure => failure(
            HouseholdAdministrationFailureCategory::NetworkFailure,
            "Luna could not reach OpenAI.",
        ),
        ReasoningPortError::Timeout => failure(
            HouseholdAdministrationFailureCategory::Timeout,
            "OpenAI did not respond within Luna's processing limit.",
        ),
        ReasoningPortError::StructuredResponseInvalid => failure(
            HouseholdAdministrationFailureCategory::StructuredResponseInvalid,
            "OpenAI returned an invalid Household Administration result.",
        ),
        ReasoningPortError::OpenAiContractMismatch => failure(
            HouseholdAdministrationFailureCategory::OpenAiContractMismatch,
            "The OpenAI Household Administration contract is incompatible.",
        ),
        ReasoningPortError::Unavailable => failure(
            HouseholdAdministrationFailureCategory::ReasoningUnavailable,
            "Luna's Household Administration reasoning is temporarily unavailable.",
        ),
        ReasoningPortError::MalformedResult => failure(
            HouseholdAdministrationFailureCategory::MalformedProviderResult,
            "OpenAI returned an invalid Household Administration result.",
        ),
        ReasoningPortError::IncompatibleContractVersion => failure(
            HouseholdAdministrationFailureCategory::IncompatibleContractVersion,
            "The Household Administration contract version is incompatible.",
        ),
    }
}

fn source_failure(error: SourcePortError) -> HouseholdAdministrationFailure {
    match error {
        SourcePortError::TooLarge => failure(
            HouseholdAdministrationFailureCategory::SourceTooLarge,
            "The source exceeds Luna's Household Administration processing limit.",
        ),
        SourcePortError::Unsupported => failure(
            HouseholdAdministrationFailureCategory::UnsupportedSource,
            "Luna cannot process this source type yet.",
        ),
        SourcePortError::Unavailable | SourcePortError::NotFound => failure(
            HouseholdAdministrationFailureCategory::SourceUnavailable,
            "The source is no longer available for Household Administration.",
        ),
    }
}

fn persistence_failure() -> HouseholdAdministrationFailure {
    failure(
        HouseholdAdministrationFailureCategory::PersistenceUnavailable,
        "Luna could not persist the Household Administration turn.",
    )
}

fn missing_target_failure(member_message: &str) -> HouseholdAdministrationFailure {
    if is_correction(member_message) {
        failure(
            HouseholdAdministrationFailureCategory::InvalidCorrectionTarget,
            "Luna cannot safely determine which Household Work the correction targets.",
        )
    } else {
        failure(
            HouseholdAdministrationFailureCategory::MissingExistingWork,
            "The Household Work selected for this update no longer exists.",
        )
    }
}

fn is_correction(message: &str) -> bool {
    let message = message.to_ascii_lowercase();
    [
        "actually",
        "not our",
        "not the",
        "correction",
        "correct ",
        "instead",
    ]
    .iter()
    .any(|phrase| message.contains(phrase))
}

fn failure(
    category: HouseholdAdministrationFailureCategory,
    message: &str,
) -> HouseholdAdministrationFailure {
    HouseholdAdministrationFailure::new(category, message)
}
