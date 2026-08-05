mod persistence;

use std::{path::PathBuf, sync::Arc};

use axum::{
    extract::{DefaultBodyLimit, Multipart, Path, State},
    http::StatusCode,
    response::IntoResponse,
    routing::{get, post},
    Json, Router,
};
use luna_core::{
    ActionApproval, HandleHouseholdAdministrationTurn, HandleHouseholdWorkCommand,
    HouseholdAdministrationFailureCategory, HouseholdAdministrationReasoning, HouseholdContextItem,
    HouseholdWork, HouseholdWorkCommand, HouseholdWorkStatus,
    OpenAiHouseholdAdministrationReasoningAdapter, ProposedAction, WorkFact, WorkFactKey,
    MAX_MVP_DOCUMENT_BYTES,
};
use serde::{Deserialize, Serialize};
use thiserror::Error;
use time::{macros::format_description, OffsetDateTime};
use uuid::Uuid;

use persistence::{
    ExecuteTurnError, ExecutedTurn, PersistedConversationMessage, SourceMetadata, WebStore,
};

const GLOBAL_CONVERSATION_ID: i64 = 1;

#[derive(Debug, Clone)]
pub struct WebConfig {
    pub data_dir: PathBuf,
    pub household_id: String,
    pub member_id: String,
    pub member_display_name: String,
    pub household_name: String,
}

#[derive(Debug, Error)]
pub enum WebApiError {
    #[error("web persistence is unavailable")]
    Persistence,
    #[error("the source is missing")]
    MissingSource,
    #[error("the source type is unsupported")]
    UnsupportedSource,
    #[error("the source exceeds the upload limit")]
    SourceTooLarge,
    #[error("the request is invalid")]
    InvalidInput,
    #[error("the requested Household Work does not exist")]
    NotFound,
    #[error("Household Administration reasoning is unavailable")]
    Reasoning(HouseholdAdministrationFailureCategory),
}

#[derive(Clone)]
struct AppState {
    config: Arc<WebConfig>,
    store: WebStore,
    reasoning: Arc<dyn HouseholdAdministrationReasoning>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct MemberView {
    display_name: String,
    household_name: String,
    initials: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ReviewedView {
    emails: u32,
    documents: u32,
    calendar: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ConversationMessageView {
    id: String,
    role: String,
    body: String,
    created_at: String,
    #[serde(skip_serializing_if = "Vec::is_empty", default)]
    contextual_work_ids: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct HouseholdFactView {
    key: String,
    label: String,
    value: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ProposedActionView {
    id: String,
    label: String,
    description: String,
    approval_required: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct SourceView {
    label: String,
    detail: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct HouseholdWorkView {
    id: String,
    title: String,
    summary: String,
    status: String,
    source: SourceView,
    due_label: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    amount_label: Option<String>,
    household_entity: String,
    activity: String,
    recommendation: String,
    needs: Option<String>,
    facts: Vec<HouseholdFactView>,
    #[serde(skip_serializing_if = "Option::is_none")]
    proposed_action: Option<ProposedActionView>,
    #[serde(skip_serializing_if = "Option::is_none")]
    audit_history: Option<Vec<String>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct TodayBriefingView {
    member: MemberView,
    date_label: String,
    greeting: String,
    reviewed: ReviewedView,
    conversation: Vec<ConversationMessageView>,
    work: Vec<HouseholdWorkView>,
    partial_failures: Vec<serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct SourceUploadView {
    source_id: String,
    display_name: String,
    media_type: String,
    size_bytes: u64,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ConversationInput {
    message: String,
    #[serde(default)]
    contextual_work_ids: Vec<String>,
    source_id: Option<String>,
    viewing_work_id: Option<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct ClarificationView {
    question: String,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    candidate_work_ids: Vec<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct ConversationResultView {
    briefing: TodayBriefingView,
    member_message: ConversationMessageView,
    luna_message: ConversationMessageView,
    affected_work_ids: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    clarification: Option<ClarificationView>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct FactCorrectionInput {
    fact_key: String,
    value: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct MutationResultView {
    briefing: TodayBriefingView,
    work: Option<HouseholdWorkView>,
    confirmation: String,
}

pub fn app(config: WebConfig) -> Result<Router, WebApiError> {
    let reasoning = OpenAiHouseholdAdministrationReasoningAdapter::from_env()
        .map_err(|error| WebApiError::Reasoning(error_category(error)))?;
    app_with_reasoning(config, Arc::new(reasoning))
}

pub fn app_with_reasoning(
    config: WebConfig,
    reasoning: Arc<dyn HouseholdAdministrationReasoning>,
) -> Result<Router, WebApiError> {
    if config.household_id.trim().is_empty() || config.member_id.trim().is_empty() {
        return Err(WebApiError::InvalidInput);
    }
    let store = WebStore::open(&config.data_dir).map_err(|_| WebApiError::Persistence)?;
    let state = AppState {
        config: Arc::new(config),
        store,
        reasoning,
    };
    Ok(Router::new()
        .route("/api/today", get(get_today))
        .route("/api/household-work/{id}", get(get_work_item))
        .route("/api/conversation", post(conversation_turn))
        .route("/api/sources", post(upload_source))
        .route(
            "/api/household-work/{id}/approve/{action_id}",
            post(approve_action),
        )
        .route("/api/household-work/{id}/dismiss", post(dismiss_work))
        .route("/api/household-work/{id}/complete", post(complete_work))
        .route("/api/household-work/{id}/facts", post(correct_fact))
        .layer(DefaultBodyLimit::max(
            MAX_MVP_DOCUMENT_BYTES as usize + 64 * 1024,
        ))
        .with_state(state))
}

async fn get_today(State(state): State<AppState>) -> Result<Json<TodayBriefingView>, WebApiError> {
    Ok(Json(project_today(&state, false)?))
}

async fn get_work_item(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<Json<HouseholdWorkView>, WebApiError> {
    let work = state
        .store
        .find_work(&state.config.household_id, &id)
        .map_err(|_| WebApiError::Persistence)?
        .ok_or(WebApiError::NotFound)?;
    Ok(Json(project_work(&state, &work, true)?))
}

async fn conversation_turn(
    State(state): State<AppState>,
    Json(input): Json<ConversationInput>,
) -> Result<Json<ConversationResultView>, WebApiError> {
    execute_conversation(state, input).await.map(Json)
}

async fn execute_conversation(
    state: AppState,
    input: ConversationInput,
) -> Result<ConversationResultView, WebApiError> {
    let message = if input.message.trim().is_empty() && input.source_id.is_some() {
        "Take care of this.".to_owned()
    } else {
        input.message.trim().to_owned()
    };
    if message.is_empty() {
        return Err(WebApiError::InvalidInput);
    }
    let active_reference = input
        .contextual_work_ids
        .first()
        .cloned()
        .or(input.viewing_work_id);
    let turn_input = HandleHouseholdAdministrationTurn {
        household_id: state.config.household_id.clone(),
        conversation_id: GLOBAL_CONVERSATION_ID,
        member_message: message,
        source_reference: input.source_id,
        active_work_reference: active_reference,
        authorised_household_context: vec![
            HouseholdContextItem {
                category: "member".to_owned(),
                value: state.config.member_display_name.clone(),
                source_reference: "server-session".to_owned(),
            },
            HouseholdContextItem {
                category: "household".to_owned(),
                value: state.config.household_name.clone(),
                source_reference: "server-session".to_owned(),
            },
        ],
        available_actions: Vec::new(),
        authorised_actor: state.config.member_id.clone(),
        request_id: format!("web-{}", Uuid::new_v4()),
    };
    let contextual_ids = input.contextual_work_ids;
    let store = state.store.clone();
    let reasoning = state.reasoning.clone();
    let executed = tokio::task::spawn_blocking(move || {
        store.execute_turn(reasoning.as_ref(), turn_input, contextual_ids)
    })
    .await
    .map_err(|_| WebApiError::Persistence)?
    .map_err(map_turn_error)?;
    conversation_result(&state, executed)
}

async fn approve_action(
    State(state): State<AppState>,
    Path((id, action_id)): Path<(String, String)>,
) -> Result<Json<MutationResultView>, WebApiError> {
    run_work_command(state, id, HouseholdWorkCommand::ApproveAction { action_id })
        .await
        .map(Json)
}

async fn dismiss_work(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<Json<MutationResultView>, WebApiError> {
    run_work_command(state, id, HouseholdWorkCommand::Dismiss)
        .await
        .map(Json)
}

async fn complete_work(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<Json<MutationResultView>, WebApiError> {
    run_work_command(state, id, HouseholdWorkCommand::Complete)
        .await
        .map(Json)
}

async fn correct_fact(
    State(state): State<AppState>,
    Path(id): Path<String>,
    Json(input): Json<FactCorrectionInput>,
) -> Result<Json<MutationResultView>, WebApiError> {
    if input.fact_key.trim().is_empty() || input.value.trim().is_empty() {
        return Err(WebApiError::InvalidInput);
    }
    let key = parse_fact_key(input.fact_key.trim()).ok_or(WebApiError::InvalidInput)?;
    run_work_command(
        state,
        id,
        HouseholdWorkCommand::CorrectFact {
            key,
            value: input.value,
        },
    )
    .await
    .map(Json)
}

async fn run_work_command(
    state: AppState,
    work_id: String,
    command: HouseholdWorkCommand,
) -> Result<MutationResultView, WebApiError> {
    if state
        .store
        .find_work(&state.config.household_id, &work_id)
        .map_err(|_| WebApiError::Persistence)?
        .is_none()
    {
        return Err(WebApiError::NotFound);
    }
    let command_input = HandleHouseholdWorkCommand {
        household_id: state.config.household_id.clone(),
        conversation_id: GLOBAL_CONVERSATION_ID,
        work_id: work_id.clone(),
        command,
        authorised_actor: state.config.member_id.clone(),
        request_id: format!("web-command-{}", Uuid::new_v4()),
    };
    let store = state.store.clone();
    let reasoning = state.reasoning.clone();
    let executed = tokio::task::spawn_blocking(move || {
        store.execute_command(reasoning.as_ref(), command_input)
    })
    .await
    .map_err(|_| WebApiError::Persistence)?
    .map_err(map_turn_error)?;
    let confirmation = executed.outcome.message;
    let briefing = project_today(&state, false)?;
    let work = state
        .store
        .find_work(&state.config.household_id, &work_id)
        .map_err(|_| WebApiError::Persistence)?
        .map(|work| project_work(&state, &work, true))
        .transpose()?;
    Ok(MutationResultView {
        briefing,
        work,
        confirmation,
    })
}

async fn upload_source(
    State(state): State<AppState>,
    mut multipart: Multipart,
) -> Result<(StatusCode, Json<SourceUploadView>), WebApiError> {
    while let Some(field) = multipart
        .next_field()
        .await
        .map_err(|_| WebApiError::MissingSource)?
    {
        if field.name() != Some("source") {
            continue;
        }
        let media_type = field
            .content_type()
            .map(str::to_owned)
            .ok_or(WebApiError::UnsupportedSource)?;
        if !matches!(
            media_type.as_str(),
            "application/pdf" | "image/jpeg" | "image/png"
        ) {
            return Err(WebApiError::UnsupportedSource);
        }
        let display_name = safe_display_name(field.file_name());
        let bytes = field
            .bytes()
            .await
            .map_err(|_| WebApiError::SourceTooLarge)?;
        if bytes.len() as u64 > MAX_MVP_DOCUMENT_BYTES {
            return Err(WebApiError::SourceTooLarge);
        }
        let stored = state
            .store
            .store_source(
                &state.config.household_id,
                &display_name,
                &media_type,
                &bytes,
                &now(),
            )
            .map_err(|_| WebApiError::Persistence)?;
        return Ok((
            StatusCode::CREATED,
            Json(SourceUploadView {
                source_id: stored.id,
                display_name: stored.display_name,
                media_type: stored.media_type,
                size_bytes: stored.size_bytes,
            }),
        ));
    }
    Err(WebApiError::MissingSource)
}

fn conversation_result(
    state: &AppState,
    executed: ExecutedTurn,
) -> Result<ConversationResultView, WebApiError> {
    let candidates = executed
        .outcome
        .work
        .as_ref()
        .map(|work| vec![work.id.clone()])
        .unwrap_or_default();
    Ok(ConversationResultView {
        briefing: project_today(state, false)?,
        member_message: project_message(executed.member_message),
        luna_message: project_message(executed.luna_message),
        affected_work_ids: executed.affected_work_ids,
        clarification: executed
            .outcome
            .clarification
            .map(|clarification| ClarificationView {
                question: clarification.question,
                candidate_work_ids: candidates,
            }),
    })
}

fn project_today(
    state: &AppState,
    include_history: bool,
) -> Result<TodayBriefingView, WebApiError> {
    let document_count = state
        .store
        .source_count(&state.config.household_id)
        .map_err(|_| WebApiError::Persistence)?;
    let conversation = state
        .store
        .list_conversation(&state.config.household_id, GLOBAL_CONVERSATION_ID)
        .map_err(|_| WebApiError::Persistence)?
        .into_iter()
        .map(project_message)
        .collect();
    let work = state
        .store
        .list_work(&state.config.household_id)
        .map_err(|_| WebApiError::Persistence)?
        .into_iter()
        .filter(|work| work.status.is_open())
        .map(|work| project_work(state, &work, include_history))
        .collect::<Result<Vec<_>, _>>()?;
    Ok(TodayBriefingView {
        member: MemberView {
            display_name: state.config.member_display_name.clone(),
            household_name: state.config.household_name.clone(),
            initials: initials(&state.config.member_display_name),
        },
        date_label: date_label(),
        greeting: greeting(),
        reviewed: ReviewedView {
            emails: 0,
            documents: document_count,
            calendar: false,
        },
        conversation,
        work,
        partial_failures: Vec::new(),
    })
}

fn project_message(message: PersistedConversationMessage) -> ConversationMessageView {
    ConversationMessageView {
        id: message.id,
        role: message.role,
        body: message.body,
        created_at: message.created_at,
        contextual_work_ids: message.contextual_work_ids,
    }
}

fn project_work(
    state: &AppState,
    work: &HouseholdWork,
    include_history: bool,
) -> Result<HouseholdWorkView, WebApiError> {
    let source = work
        .source_refs
        .iter()
        .find(|reference| reference.starts_with("source-"))
        .map(|reference| {
            state
                .store
                .source_metadata(&state.config.household_id, reference)
        })
        .transpose()
        .map_err(|_| WebApiError::Persistence)?
        .flatten();
    let amount = fact_value(&work.facts, WorkFactKey::Amount);
    let entity = fact_value(&work.facts, WorkFactKey::Property)
        .unwrap_or_else(|| state.config.household_name.clone());
    let proposed = work
        .proposed_actions
        .iter()
        .find(|action| action.approval == ActionApproval::Required)
        .map(project_action);
    let source_view = source.map(project_source).unwrap_or(SourceView {
        label: "Household conversation".to_owned(),
        detail: "Member-provided household context".to_owned(),
    });
    Ok(HouseholdWorkView {
        id: work.id.clone(),
        title: work.summary.clone(),
        summary: work.summary.clone(),
        status: project_status(work.status).to_owned(),
        source: source_view,
        due_label: work
            .due_at
            .as_ref()
            .map(|due| format!("Due {due}"))
            .unwrap_or_else(|| "No due date".to_owned()),
        amount_label: amount,
        household_entity: entity,
        activity: activity(work),
        recommendation: recommendation(work),
        needs: needs(work),
        facts: work.facts.iter().map(project_fact).collect(),
        proposed_action: proposed,
        audit_history: include_history.then(|| work.audit_events.clone()),
    })
}

fn project_source(source: SourceMetadata) -> SourceView {
    SourceView {
        label: source.display_name,
        detail: format!(
            "Uploaded {} · {} bytes · {}",
            source.media_type, source.size_bytes, source.created_at
        ),
    }
}

fn project_fact(fact: &WorkFact) -> HouseholdFactView {
    let key = fact_key(fact.key.clone());
    HouseholdFactView {
        key: key.to_owned(),
        label: label(key),
        value: fact.value.clone(),
    }
}

fn project_action(action: &ProposedAction) -> ProposedActionView {
    ProposedActionView {
        id: action.id.clone(),
        label: action.summary.clone(),
        description: action.summary.clone(),
        approval_required: matches!(action.approval, ActionApproval::Required),
    }
}

fn recommendation(work: &HouseholdWork) -> String {
    work.proposed_actions
        .first()
        .map(|action| action.summary.clone())
        .unwrap_or_else(|| {
            if work.status.is_terminal() {
                "No further action is needed."
            } else {
                "Continue through the household conversation."
            }
            .to_owned()
        })
}

fn activity(work: &HouseholdWork) -> String {
    match work.status {
        HouseholdWorkStatus::AwaitingApproval => {
            "Luna prepared an action and is waiting for approval.".to_owned()
        }
        HouseholdWorkStatus::NeedsClarification => {
            "Luna needs one clarification before continuing.".to_owned()
        }
        HouseholdWorkStatus::Monitoring => "An approved action is being kept in view.".to_owned(),
        HouseholdWorkStatus::Completed => "This Household Work is complete.".to_owned(),
        HouseholdWorkStatus::Dismissed | HouseholdWorkStatus::NoLongerRelevant => {
            "This Household Work no longer needs attention.".to_owned()
        }
        _ => "Luna is keeping this Household Work in view.".to_owned(),
    }
}

fn needs(work: &HouseholdWork) -> Option<String> {
    match work.status {
        HouseholdWorkStatus::AwaitingApproval => Some("Your approval".to_owned()),
        HouseholdWorkStatus::NeedsClarification => Some("Your clarification".to_owned()),
        HouseholdWorkStatus::Blocked => Some("A blocking issue needs attention".to_owned()),
        status if status.is_terminal() => None,
        _ => Some("Review Luna's recommendation".to_owned()),
    }
}

fn fact_value(facts: &[WorkFact], wanted: WorkFactKey) -> Option<String> {
    facts
        .iter()
        .find(|fact| fact.key == wanted)
        .map(|fact| fact.value.clone())
}

fn fact_key(key: WorkFactKey) -> &'static str {
    match key {
        WorkFactKey::Provider => "provider",
        WorkFactKey::Property => "property",
        WorkFactKey::Account => "account",
        WorkFactKey::Amount => "amount",
        WorkFactKey::DueDate => "dueDate",
        WorkFactKey::RequiredAction => "requiredAction",
        WorkFactKey::Urgency => "urgency",
        WorkFactKey::Other => "other",
    }
}

fn parse_fact_key(key: &str) -> Option<WorkFactKey> {
    match key {
        "provider" => Some(WorkFactKey::Provider),
        "property" => Some(WorkFactKey::Property),
        "account" => Some(WorkFactKey::Account),
        "amount" => Some(WorkFactKey::Amount),
        "dueDate" | "due_date" => Some(WorkFactKey::DueDate),
        "requiredAction" | "required_action" => Some(WorkFactKey::RequiredAction),
        "urgency" => Some(WorkFactKey::Urgency),
        "other" => Some(WorkFactKey::Other),
        _ => None,
    }
}

fn label(key: &str) -> String {
    match key {
        "dueDate" => "Due date",
        "requiredAction" => "Required action",
        other => other,
    }
    .to_owned()
    .replace('_', " ")
    .split_whitespace()
    .map(|word| {
        let mut chars = word.chars();
        chars
            .next()
            .map(|first| first.to_uppercase().collect::<String>() + chars.as_str())
            .unwrap_or_default()
    })
    .collect::<Vec<_>>()
    .join(" ")
}

fn project_status(status: HouseholdWorkStatus) -> &'static str {
    match status {
        HouseholdWorkStatus::AwaitingApproval => "awaitingApproval",
        HouseholdWorkStatus::NeedsClarification => "needsClarification",
        HouseholdWorkStatus::Completed => "completed",
        HouseholdWorkStatus::Dismissed | HouseholdWorkStatus::NoLongerRelevant => "dismissed",
        HouseholdWorkStatus::Monitoring => "upcoming",
        HouseholdWorkStatus::Active
        | HouseholdWorkStatus::InProgress
        | HouseholdWorkStatus::Blocked => "needsAttention",
    }
}

fn map_turn_error(error: ExecuteTurnError) -> WebApiError {
    match error {
        ExecuteTurnError::Persistence => WebApiError::Persistence,
        ExecuteTurnError::Engine(error) => WebApiError::Reasoning(error.category),
    }
}

fn error_category(error: luna_core::ReasoningPortError) -> HouseholdAdministrationFailureCategory {
    use luna_core::ReasoningPortError;
    match error {
        ReasoningPortError::MissingApiKey => HouseholdAdministrationFailureCategory::MissingApiKey,
        ReasoningPortError::InvalidApiKey => HouseholdAdministrationFailureCategory::InvalidApiKey,
        ReasoningPortError::ModelUnavailable => {
            HouseholdAdministrationFailureCategory::ModelUnavailable
        }
        ReasoningPortError::RateLimited => HouseholdAdministrationFailureCategory::RateLimited,
        ReasoningPortError::RequestTooLarge => {
            HouseholdAdministrationFailureCategory::RequestTooLarge
        }
        ReasoningPortError::UnsupportedMedia => {
            HouseholdAdministrationFailureCategory::UnsupportedMedia
        }
        ReasoningPortError::NetworkFailure => {
            HouseholdAdministrationFailureCategory::NetworkFailure
        }
        ReasoningPortError::Timeout => HouseholdAdministrationFailureCategory::Timeout,
        ReasoningPortError::StructuredResponseInvalid => {
            HouseholdAdministrationFailureCategory::StructuredResponseInvalid
        }
        ReasoningPortError::OpenAiContractMismatch => {
            HouseholdAdministrationFailureCategory::OpenAiContractMismatch
        }
        ReasoningPortError::Unavailable => {
            HouseholdAdministrationFailureCategory::ReasoningUnavailable
        }
        ReasoningPortError::MalformedResult => {
            HouseholdAdministrationFailureCategory::MalformedProviderResult
        }
        ReasoningPortError::IncompatibleContractVersion => {
            HouseholdAdministrationFailureCategory::IncompatibleContractVersion
        }
    }
}

fn initials(name: &str) -> String {
    name.split_whitespace()
        .filter_map(|part| part.chars().next())
        .take(2)
        .collect::<String>()
        .to_uppercase()
}
fn date_label() -> String {
    OffsetDateTime::now_utc()
        .format(&format_description!("[weekday], [day] [month repr:long]"))
        .unwrap_or_else(|_| "Today".to_owned())
}
fn now() -> String {
    OffsetDateTime::now_utc()
        .format(&time::format_description::well_known::Rfc3339)
        .unwrap_or_else(|_| OffsetDateTime::now_utc().unix_timestamp_nanos().to_string())
}

fn safe_display_name(filename: Option<&str>) -> String {
    let leaf = filename
        .and_then(|value| std::path::Path::new(value).file_name())
        .and_then(|value| value.to_str())
        .unwrap_or("household-document");
    let name: String = leaf
        .chars()
        .filter(|character| !character.is_control())
        .map(|character| {
            if matches!(character, '/' | '\\' | ':') {
                '_'
            } else {
                character
            }
        })
        .take(120)
        .collect();
    if name.trim().is_empty() {
        "household-document".to_owned()
    } else {
        name
    }
}

fn greeting() -> String {
    match OffsetDateTime::now_utc().hour() {
        0..=11 => "Good morning",
        12..=17 => "Good afternoon",
        _ => "Good evening",
    }
    .to_owned()
}

impl IntoResponse for WebApiError {
    fn into_response(self) -> axum::response::Response {
        let (status, category, message) = match self {
            Self::MissingSource | Self::InvalidInput => (
                StatusCode::BAD_REQUEST,
                "invalidInput",
                "Add a message or choose a PDF, JPG or PNG household document.",
            ),
            Self::UnsupportedSource => (
                StatusCode::UNSUPPORTED_MEDIA_TYPE,
                "unsupportedSource",
                "Choose a PDF, JPG or PNG household document.",
            ),
            Self::SourceTooLarge => (
                StatusCode::PAYLOAD_TOO_LARGE,
                "sourceTooLarge",
                "That document is larger than the 5 MB source limit.",
            ),
            Self::NotFound => (
                StatusCode::NOT_FOUND,
                "notFound",
                "That Household Work is no longer available.",
            ),
            Self::Persistence => (
                StatusCode::SERVICE_UNAVAILABLE,
                "persistenceUnavailable",
                "Luna is temporarily unavailable. Your Household Work is safe.",
            ),
            Self::Reasoning(category) => (
                StatusCode::SERVICE_UNAVAILABLE,
                failure_category(category),
                "Luna could not safely complete that turn. Your Household Work was not changed.",
            ),
        };
        (
            status,
            Json(serde_json::json!({ "error": { "category": category, "message": message } })),
        )
            .into_response()
    }
}

fn failure_category(category: HouseholdAdministrationFailureCategory) -> &'static str {
    match category {
        HouseholdAdministrationFailureCategory::MissingApiKey => "missingApiKey",
        HouseholdAdministrationFailureCategory::InvalidApiKey => "invalidApiKey",
        HouseholdAdministrationFailureCategory::ModelUnavailable => "modelUnavailable",
        HouseholdAdministrationFailureCategory::RateLimited => "rateLimited",
        HouseholdAdministrationFailureCategory::PersistenceUnavailable => "persistenceUnavailable",
        HouseholdAdministrationFailureCategory::SourceTooLarge => "sourceTooLarge",
        HouseholdAdministrationFailureCategory::UnsupportedSource
        | HouseholdAdministrationFailureCategory::UnsupportedMedia => "unsupportedSource",
        HouseholdAdministrationFailureCategory::InvalidInput => "invalidInput",
        _ => "reasoningUnavailable",
    }
}
