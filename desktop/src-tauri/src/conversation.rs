use std::{
    env,
    fs::{self, OpenOptions},
    io::{self, Read, Write},
    panic::{catch_unwind, AssertUnwindSafe},
    path::{Path, PathBuf},
    process::Command,
    sync::Arc,
    time::{SystemTime, UNIX_EPOCH},
};

use image::ImageFormat;
use rusqlite::{params, Connection, OptionalExtension};
use serde::{de::DeserializeOwned, Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

use crate::cabinet::ensure_incoming_folder;
use crate::household_administration::{apply_result, HouseholdAdministrationFailureCategory};
use crate::household_work::HouseholdWork;
use crate::intelligence::{
    CandidateDirectionInterpretation, CloudConsentDecision, HouseholdAdministrationResult,
    IntelligenceResult,
};
use crate::portable_memory::{
    PortableDocumentRelationshipKind, PortableHistoryEvent, PortableReference,
};
use crate::portable_memory::{PortableFilingRuleDefinition, PortableFilingRuleState};
use crate::trusted_device::{
    CredentialVault, ProtectedHouseholdState, TrustedDeviceError, TrustedDeviceManager,
};

pub const MAX_MVP_DOCUMENT_BYTES: u64 = 5 * 1024 * 1024;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Conversation {
    pub id: i64,
    pub household_id: String,
    pub title: String,
    pub archived: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum DocumentProcessingState {
    NeedsCloudConsent,
    InspectingWithAssistance,
    WaitingForCloudAssistance,
    NeedsMemberDirection,
    PossibleDuplicate,
    ReadyToFile,
    Filing,
    CabinetUnavailable,
    Filed,
    Dismissed,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum DuplicateKind {
    Exact,
    Possible,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum DuplicateDecision {
    KeepBoth,
    LinkCopies,
    DiscardNew,
    UpdatedVersion,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DuplicateCandidate {
    pub arrival_id: i64,
    pub kind: DuplicateKind,
    pub original_name: String,
    pub checksum: String,
    pub filed_destination: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DuplicateReview {
    pub candidates: Vec<DuplicateCandidate>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DuplicateResolution {
    pub decision: DuplicateDecision,
    pub related_arrival_id: i64,
    pub related_original_name: String,
    #[serde(default)]
    pub duplicate_kind: Option<DuplicateKind>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RebuiltDocumentRelationship {
    pub event_id: String,
    pub document_reference: PortableReference,
    pub related_document_reference: PortableReference,
    pub relationship: PortableDocumentRelationshipKind,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum DuplicateAuditKind {
    DuplicateDecisionRecorded,
    DuplicatePreferenceApplied,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DuplicateAuditEvent {
    pub id: i64,
    pub household_id: String,
    pub kind: DuplicateAuditKind,
    pub decision: DuplicateDecision,
    pub subject: String,
    pub outcome: String,
    pub related_arrival_id: i64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ConfidenceState {
    Confirmed,
    LooksRight,
    NeedsChecking,
    Unknown,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ReviewEvidence {
    pub label: String,
    pub value: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ReviewField {
    pub value: Option<String>,
    pub confidence_state: ConfidenceState,
}

impl Default for ReviewField {
    fn default() -> Self {
        Self {
            value: None,
            confidence_state: ConfidenceState::Unknown,
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DocumentContextReview {
    pub document_type: ReviewField,
    pub service_provider: ReviewField,
    pub service_provider_relevance: ReviewField,
    pub addressee: ReviewField,
    pub property: ReviewField,
    pub property_relevance: ReviewField,
    pub account: ReviewField,
    pub amount: ReviewField,
    pub relevant_dates: Vec<ReviewField>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ClarificationQuestion {
    pub field: ContextField,
    pub prompt: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ContextField {
    DocumentType,
    ServiceProvider,
    ServiceProviderRelevance,
    Addressee,
    Property,
    PropertyRelevance,
    Account,
    Amount,
    RelevantDates,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ConversationPromptPurpose {
    ClarifyContext,
    ConfirmFilingDecision,
    ChooseCloudAssistance,
    ResolveDuplicate,
    LearnFilingRule,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ConversationExpectedResponse {
    Confirmation,
    ContextValue,
    Choice,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ConversationAction {
    Yes,
    No,
    KeepLocal,
    KeepBoth,
    LinkCopies,
    DiscardNew,
    UpdatedVersion,
    AlwaysDoThis,
    ReviewDetails,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ConversationPrompt {
    pub id: String,
    pub purpose: ConversationPromptPurpose,
    pub subject: String,
    pub message: String,
    pub expected_response: ConversationExpectedResponse,
    pub allowed_actions: Vec<ConversationAction>,
    pub linked_document_arrival: i64,
    pub evidence_summary: Vec<String>,
    pub context_field: Option<ContextField>,
    pub related_arrival_id: Option<i64>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MemberUtterance {
    pub conversation_id: i64,
    pub message: String,
    pub linked_prompt: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum MemberDirectionCommand {
    ConfirmContextField {
        field: ContextField,
    },
    RejectContextField {
        field: ContextField,
    },
    SetContextField {
        field: ContextField,
        value: Option<String>,
    },
    ConfirmFilingDecision,
    UseCloudAssistance {
        consent: CloudConsentDecision,
    },
    ResolveDuplicate {
        decision: DuplicateDecision,
        related_arrival_id: i64,
    },
    LearnFilingRule,
    Decline,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum InterpretationConfidence {
    Confident,
    Ambiguous,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DirectionInterpretation {
    pub proposed_commands: Vec<MemberDirectionCommand>,
    pub confidence: InterpretationConfidence,
    pub ambiguity: Option<String>,
    pub evidence: Vec<String>,
}

pub trait MemberDirectionInterpreter: Send + Sync {
    fn interpret(
        &self,
        prompt: &ConversationPrompt,
        utterance: &MemberUtterance,
    ) -> DirectionInterpretation;
}

#[derive(Default)]
pub struct DeterministicMemberDirectionInterpreter;

impl MemberDirectionInterpreter for DeterministicMemberDirectionInterpreter {
    fn interpret(
        &self,
        prompt: &ConversationPrompt,
        utterance: &MemberUtterance,
    ) -> DirectionInterpretation {
        let message = utterance.message.trim();
        let normalized = message
            .trim_matches(|character: char| character.is_ascii_punctuation())
            .to_ascii_lowercase()
            .replace(['’', '‘'], "'");
        let affirmative = matches!(
            normalized.as_str(),
            "yes" | "yep" | "yeah" | "correct" | "right" | "that's right" | "that is right"
        ) || normalized.starts_with("yes ")
            || normalized.starts_with("yes,")
            || normalized.contains("that's right")
            || normalized.contains("that is right");
        let negative = matches!(
            normalized.as_str(),
            "no" | "nope" | "not right" | "that's wrong" | "that is wrong"
        );
        let hedged = ["maybe", "perhaps", "not sure", "unsure", "i suppose"]
            .iter()
            .any(|phrase| normalized.contains(phrase));
        let delegation_request = [
            "take care of this",
            "handle this",
            "file this",
            "organise this",
            "organize this",
        ]
        .iter()
        .any(|phrase| normalized.contains(phrase));
        let ambiguous = |reason: &str| DirectionInterpretation {
            proposed_commands: Vec::new(),
            confidence: InterpretationConfidence::Ambiguous,
            ambiguity: Some(reason.to_owned()),
            evidence: vec![format!("Member said: {message}")],
        };
        let confident = |command| DirectionInterpretation {
            proposed_commands: vec![command],
            confidence: InterpretationConfidence::Confident,
            ambiguity: None,
            evidence: vec![format!("Member said: {message}")],
        };
        if delegation_request {
            return ambiguous(
                "I can take care of it, but I still need an answer to the current question.",
            );
        }
        let explicitly_negated = normalized
            .split(|character: char| !character.is_alphanumeric() && character != '\'')
            .any(|word| {
                matches!(
                    word,
                    "no" | "not"
                        | "never"
                        | "don't"
                        | "dont"
                        | "won't"
                        | "wont"
                        | "can't"
                        | "cant"
                        | "cannot"
                )
            });
        if affirmative && explicitly_negated {
            return ambiguous(
                "Luna found both a confirmation and a refusal. Please answer the current question again.",
            );
        }

        match prompt.purpose {
            ConversationPromptPurpose::ConfirmFilingDecision => {
                if affirmative {
                    confident(MemberDirectionCommand::ConfirmFilingDecision)
                } else if negative {
                    confident(MemberDirectionCommand::Decline)
                } else {
                    ambiguous("Luna could not tell whether the Filing Decision was confirmed.")
                }
            }
            ConversationPromptPurpose::LearnFilingRule => {
                if affirmative || normalized == "always do this" {
                    confident(MemberDirectionCommand::LearnFilingRule)
                } else if negative {
                    confident(MemberDirectionCommand::Decline)
                } else {
                    ambiguous("Luna could not tell whether this should become a Filing Rule.")
                }
            }
            ConversationPromptPurpose::ChooseCloudAssistance => {
                if matches!(
                    normalized.as_str(),
                    "keep local" | "keep it local" | "local only" | "stay local"
                ) || normalized.contains("keep it local")
                    || normalized.contains("keep local")
                    || normalized.contains("don't use cloud")
                    || normalized.contains("do not use cloud")
                {
                    confident(MemberDirectionCommand::UseCloudAssistance {
                        consent: CloudConsentDecision::KeepLocal,
                    })
                } else if matches!(
                    normalized.as_str(),
                    "allow once" | "use cloud once" | "one time"
                ) {
                    confident(MemberDirectionCommand::UseCloudAssistance {
                        consent: CloudConsentDecision::AllowOnce,
                    })
                } else if matches!(
                    normalized.as_str(),
                    "allow this scoped future use"
                        | "allow for this scope"
                        | "remember this consent"
                ) {
                    confident(MemberDirectionCommand::UseCloudAssistance {
                        consent: CloudConsentDecision::AllowForScope,
                    })
                } else if matches!(
                    normalized.as_str(),
                    "use existing consent grant"
                        | "use the existing consent grant"
                        | "use existing scope"
                ) {
                    confident(MemberDirectionCommand::UseCloudAssistance {
                        consent: CloudConsentDecision::UseExistingScope,
                    })
                } else {
                    ambiguous(
                        "Choose a disclosed provider and consent option below, or say “Keep local”.",
                    )
                }
            }
            ConversationPromptPurpose::ResolveDuplicate => {
                let Some(related_arrival_id) = prompt.related_arrival_id else {
                    return ambiguous("The duplicate question is no longer available.");
                };
                let decision = match normalized.as_str() {
                    "keep both" => Some(DuplicateDecision::KeepBoth),
                    "link copies" | "link the copies" => Some(DuplicateDecision::LinkCopies),
                    "discard new" | "discard the new one" | "discard the new copy" => {
                        Some(DuplicateDecision::DiscardNew)
                    }
                    "updated version"
                    | "mark as updated version"
                    | "mark it as updated version" => Some(DuplicateDecision::UpdatedVersion),
                    _ => None,
                };
                decision.map_or_else(
                    || {
                        ambiguous(
                            "Say “Keep both”, “Link copies”, “Discard new”, or “Updated version”.",
                        )
                    },
                    |decision| {
                        confident(MemberDirectionCommand::ResolveDuplicate {
                            decision,
                            related_arrival_id,
                        })
                    },
                )
            }
            ConversationPromptPurpose::ClarifyContext => {
                let Some(field) = prompt.context_field else {
                    return ambiguous("The current question is not linked to Household Context.");
                };
                if hedged {
                    return ambiguous("Luna needs a clearer answer to the current question.");
                }
                if prompt.expected_response == ConversationExpectedResponse::Confirmation {
                    if affirmative {
                        return confident(MemberDirectionCommand::ConfirmContextField { field });
                    }
                    if negative {
                        return confident(MemberDirectionCommand::RejectContextField { field });
                    }
                } else if negative
                    && matches!(field, ContextField::Property | ContextField::Account)
                {
                    return confident(MemberDirectionCommand::SetContextField {
                        field,
                        value: None,
                    });
                }

                let correction = conversational_value(message);
                if correction.is_empty() || affirmative || negative {
                    ambiguous("Luna needs a clearer answer to the current question.")
                } else {
                    confident(MemberDirectionCommand::SetContextField {
                        field,
                        value: Some(normalize_conversational_context_value(field, correction)),
                    })
                }
            }
        }
    }
}

fn normalize_conversational_context_value(field: ContextField, value: String) -> String {
    if field == ContextField::DocumentType {
        let lower = value.to_ascii_lowercase();
        for article in ["a ", "an "] {
            if lower.starts_with(article) {
                return value[article.len()..].trim().to_owned();
            }
        }
    }
    value
}

fn conversational_value(message: &str) -> String {
    let trimmed = message.trim();
    let lower = trimmed.to_ascii_lowercase();
    for marker in ["no, it is ", "no, it's ", "it is ", "it's "] {
        if lower.starts_with(marker) {
            return trimmed[marker.len()..]
                .trim()
                .trim_matches(|character: char| character.is_ascii_punctuation())
                .to_owned();
        }
    }
    trimmed
        .trim_matches(|character: char| character.is_ascii_punctuation())
        .to_owned()
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ConversationTurnStatus {
    AcceptedDirection,
    ActionPrepared,
    ClarificationRequired,
    ActionCompleted,
    ActionRefused,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DocumentConversationView {
    pub understanding: String,
    pub prompt: Option<ConversationPrompt>,
    pub completion_message: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ConversationTurnOutcome {
    pub status: ConversationTurnStatus,
    pub accepted_direction: Option<MemberDirectionCommand>,
    pub message: String,
    pub next_prompt: Option<ConversationPrompt>,
    pub arrival: DocumentArrival,
    pub cloud_result: Option<IntelligenceResult>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FilingDecisionReview {
    pub file_name: String,
    pub cabinet_destination: String,
    pub confirmed: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FilingDecisionDirection {
    pub file_name: String,
    pub cabinet_destination: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FilingRule {
    pub id: i64,
    pub document_type: String,
    pub service_provider: String,
    pub addressee: String,
    pub property: Option<String>,
    pub account: Option<String>,
    pub file_name: String,
    pub cabinet_destination: String,
    #[serde(default)]
    pub teacher: String,
    #[serde(default)]
    pub created_at: String,
    #[serde(default)]
    pub paused: bool,
    #[serde(default)]
    pub deleted: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FilingRuleSummary {
    pub id: i64,
    pub document_type: String,
    pub service_provider: String,
    pub addressee: String,
    pub property: Option<String>,
    pub account: Option<String>,
    pub file_name: String,
    pub cabinet_destination: String,
    pub teacher: String,
    pub created_at: String,
    pub paused: bool,
    pub deleted: bool,
    pub affected_documents: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FilingRuleUpdate {
    pub document_type: String,
    pub service_provider: String,
    pub addressee: String,
    pub property: Option<String>,
    pub account: Option<String>,
    pub file_name: String,
    pub cabinet_destination: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum FilingRuleAuditKind {
    Updated,
    Paused,
    Resumed,
    Deleted,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FilingRuleAuditEvent {
    pub id: i64,
    pub household_id: String,
    pub rule_id: i64,
    pub kind: FilingRuleAuditKind,
    pub subject: String,
    pub outcome: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FilingRuleReorganizationDocument {
    pub arrival_id: i64,
    pub original_name: String,
    pub current_destination: String,
    pub proposed_destination: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FilingRuleReorganizationPreview {
    pub rule_id: i64,
    pub proposed_directory: String,
    pub documents: Vec<FilingRuleReorganizationDocument>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ManualMoveCandidate {
    pub arrival_id: i64,
    pub original_name: String,
    pub previous_destination: String,
    pub current_destination: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FiledOriginal {
    pub arrival_id: i64,
    pub conversation_id: i64,
    pub original_name: String,
    pub final_path: PathBuf,
    pub checksum: String,
    pub source_path: PathBuf,
    pub filing_decision: FilingDecisionReview,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum AuditEventKind {
    DocumentFiled,
    ExactMatchHandledAutomatically,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum AuditAuthority {
    MemberDirection,
    FilingRule,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AuditEvent {
    pub id: i64,
    pub household_id: String,
    pub kind: AuditEventKind,
    pub authority: AuditAuthority,
    pub subject: String,
    pub outcome: String,
    pub filed_original: FiledOriginal,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DocumentContextDirection {
    pub document_type: Option<String>,
    #[serde(default)]
    pub document_type_resolved: bool,
    pub service_provider: Option<String>,
    #[serde(default)]
    pub service_provider_resolved: bool,
    pub addressee: Option<String>,
    #[serde(default)]
    pub addressee_resolved: bool,
    pub property: Option<String>,
    #[serde(default)]
    pub property_resolved: bool,
    pub account: Option<String>,
    #[serde(default)]
    pub account_resolved: bool,
    pub amount: Option<String>,
    #[serde(default)]
    pub amount_resolved: bool,
    pub relevant_dates: Vec<String>,
    #[serde(default)]
    pub relevant_dates_resolved: bool,
    pub service_provider_relevance: Option<ContextRelevanceDirection>,
    pub property_relevance: Option<ContextRelevanceDirection>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ContextRelevanceDirection {
    pub subject: String,
    pub explanation: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ReviewCard {
    pub confidence_state: ConfidenceState,
    pub evidence: Vec<ReviewEvidence>,
    pub uncertainties: Vec<String>,
    pub proposed_cabinet_destination: Option<String>,
    pub context: DocumentContextReview,
    pub questions: Vec<ClarificationQuestion>,
    pub filing_decision: Option<FilingDecisionReview>,
    pub learned_rule: Option<FilingRule>,
}

pub trait LocalOcr: Send + Sync {
    fn extract_text(&self, original: &Path, media_type: &str) -> Option<String>;
}

#[derive(Default)]
pub struct TesseractOcr;

impl LocalOcr for TesseractOcr {
    fn extract_text(&self, original: &Path, media_type: &str) -> Option<String> {
        if media_type == "application/pdf" {
            let directory = tempfile::tempdir().ok()?;
            let image_base = directory.path().join("page");
            let output = Command::new(configured_local_executable(
                "LUNA_PDFTOPPM_COMMAND",
                "pdftoppm",
            ))
            .arg("-png")
            .arg(original)
            .arg(&image_base)
            .output()
            .ok()?;
            if !output.status.success() {
                return None;
            }
            let mut pages = fs::read_dir(directory.path())
                .ok()?
                .filter_map(Result::ok)
                .map(|entry| entry.path())
                .filter(|path| path.extension().is_some_and(|extension| extension == "png"))
                .collect::<Vec<_>>();
            pages.sort();
            let text = pages
                .iter()
                .filter_map(|page| self.extract_image_text(page))
                .collect::<Vec<_>>()
                .join("\n\n");
            return (!text.is_empty()).then_some(text);
        }
        self.extract_image_text(original)
    }
}

impl TesseractOcr {
    fn extract_image_text(&self, image: &Path) -> Option<String> {
        let output = Command::new(tesseract_executable())
            .arg(image)
            .arg("stdout")
            .output()
            .ok()?;
        if !output.status.success() {
            return None;
        }
        let text = String::from_utf8(output.stdout).ok()?;
        let text = text.trim().to_owned();
        (!text.is_empty()).then_some(text)
    }
}

fn configured_local_executable(environment_variable: &str, default: &str) -> PathBuf {
    if let Some(command) = env::var_os(environment_variable) {
        return command.into();
    }
    default.into()
}

fn tesseract_executable() -> PathBuf {
    if let Some(command) = env::var_os("LUNA_TESSERACT_COMMAND") {
        return command.into();
    }
    #[cfg(target_os = "windows")]
    {
        if let Some(program_files) = env::var_os("ProgramFiles") {
            let command = PathBuf::from(program_files)
                .join("Tesseract-OCR")
                .join("tesseract.exe");
            if command.is_file() {
                return command;
            }
        }
    }
    "tesseract".into()
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DocumentArrival {
    pub id: i64,
    pub household_id: String,
    pub conversation_id: i64,
    pub original_name: String,
    pub original_path: PathBuf,
    pub source_path: PathBuf,
    pub checksum: String,
    pub media_type: String,
    pub extracted_text: Option<String>,
    pub review_card: ReviewCard,
    pub processing_state: DocumentProcessingState,
    pub filed_original: Option<FiledOriginal>,
    pub duplicate_review: Option<DuplicateReview>,
    pub duplicate_resolution: Option<DuplicateResolution>,
    pub authority_source: Option<AuditAuthority>,
    pub cloud_assistance_history: Vec<String>,
    pub execution_history: Vec<String>,
    pub filing_decision_declined: bool,
    pub filing_rule_declined: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TodoItem {
    pub arrival_id: i64,
    pub conversation_id: i64,
    pub conversation_title: String,
    pub conversation_deleted: bool,
    pub document_name: String,
    pub processing_state: DocumentProcessingState,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ConversationMessage {
    pub id: i64,
    pub conversation_id: i64,
    pub author: String,
    pub body: String,
    pub linked_document_arrival: Option<i64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct ConversationPayload {
    title: String,
    archived: bool,
    deleted: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct MessagePayload {
    author: String,
    body: String,
    #[serde(default)]
    linked_document_arrival: Option<i64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct DocumentArrivalPayload {
    original_name: String,
    #[serde(default)]
    original_path: PathBuf,
    source_path: PathBuf,
    #[serde(default)]
    checksum: String,
    media_type: String,
    extracted_text: Option<String>,
    processing_state: DocumentProcessingState,
    #[serde(default)]
    context_direction: DocumentContextDirection,
    #[serde(default)]
    filing_decision: Option<FilingDecisionReview>,
    #[serde(default)]
    learned_rule: Option<FilingRule>,
    #[serde(default)]
    automatic_rule_id: Option<i64>,
    #[serde(default)]
    filed_original: Option<FiledOriginal>,
    #[serde(default)]
    duplicate_review: Option<DuplicateReview>,
    #[serde(default)]
    duplicate_resolution: Option<DuplicateResolution>,
    #[serde(default)]
    cloud_assistance_history: Vec<String>,
    #[serde(default)]
    execution_history: Vec<String>,
    #[serde(default)]
    filing_decision_declined: bool,
    #[serde(default)]
    filing_rule_declined: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct DuplicatePreferencePayload {
    checksum: String,
    context_direction: DocumentContextDirection,
    decision: DuplicateDecision,
    related_arrival_id: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct DuplicateAuditPayload {
    kind: DuplicateAuditKind,
    decision: DuplicateDecision,
    subject: String,
    outcome: String,
    related_arrival_id: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct AuditEventPayload {
    kind: AuditEventKind,
    authority: AuditAuthority,
    subject: String,
    outcome: String,
    filed_original: FiledOriginal,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct FilingRuleAuditPayload {
    kind: FilingRuleAuditKind,
    subject: String,
    outcome: String,
}

impl DocumentArrivalPayload {
    fn restore_legacy_original_path(&mut self) {
        if self.original_path.as_os_str().is_empty() {
            self.original_path = self.source_path.clone();
        }
    }
}

#[derive(Debug, Error)]
pub enum ConversationError {
    #[error("The Conversation title cannot be empty.")]
    EmptyTitle,
    #[error("The message cannot be empty.")]
    EmptyMessage,
    #[error("The Conversation no longer exists.")]
    NotFound,
    #[error("Only PDF, JPG, and PNG documents can be attached.")]
    UnsupportedDocument,
    #[error("The selected document is larger than Luna's 5 MiB MVP processing limit.")]
    DocumentTooLarge,
    #[error("The selected document does not match its declared file type.")]
    InvalidDocument,
    #[error("A different Original already occupies this document's preserved location.")]
    OriginalConflict,
    #[error("A different Original already occupies the Cabinet Destination.")]
    CabinetDestinationConflict,
    #[error("The staged or filed Original could not be verified.")]
    OriginalVerificationFailed,
    #[error("The duplicate decision is no longer available.")]
    DuplicateDecisionUnavailable,
    #[error("Household Context must be resolved before confirming a Filing Decision.")]
    UnresolvedContext,
    #[error("The candidate Direction Interpretation violates Document Handling constraints.")]
    InvalidDirectionInterpretation,
    #[error("The conversational question is no longer current.")]
    StaleConversationPrompt,
    #[error("The interpreted Member Direction does not answer the current question.")]
    InvalidMemberDirection,
    #[error(
        "The requested Household Work transition is not authorised by validated member direction."
    )]
    InvalidHouseholdWorkTransition,
    #[error("The Cabinet Destination must be a safe relative path ending in the chosen filename.")]
    InvalidCabinetDestination,
    #[error("The selected document is unavailable.")]
    DocumentUnavailable(#[from] io::Error),
    #[error("Protected Household state is unavailable.")]
    TrustedDevice(#[from] TrustedDeviceError),
    #[error("Protected Household state is invalid.")]
    ProtectedState(#[from] serde_json::Error),
    #[error("Conversation storage is unavailable.")]
    Storage(#[from] rusqlite::Error),
}

#[derive(Clone)]
pub struct ConversationStore<V: CredentialVault> {
    database: PathBuf,
    trusted_device: TrustedDeviceManager<V>,
    local_ocr: Arc<dyn LocalOcr>,
}

impl<V: CredentialVault> ConversationStore<V> {
    pub fn open(
        database: impl AsRef<Path>,
        trusted_device: TrustedDeviceManager<V>,
    ) -> Result<Self, ConversationError> {
        Self::open_with_ocr(database, trusted_device, TesseractOcr)
    }

    pub fn open_with_ocr(
        database: impl AsRef<Path>,
        trusted_device: TrustedDeviceManager<V>,
        local_ocr: impl LocalOcr + 'static,
    ) -> Result<Self, ConversationError> {
        let store = Self {
            database: database.as_ref().to_owned(),
            trusted_device,
            local_ocr: Arc::new(local_ocr),
        };
        store.connect()?.execute_batch(
            "CREATE TABLE IF NOT EXISTS conversations (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                household_id TEXT NOT NULL,
                protected_payload TEXT NOT NULL
            );
            CREATE INDEX IF NOT EXISTS conversations_household
                ON conversations(household_id);
            CREATE TABLE IF NOT EXISTS conversation_messages (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                conversation_id INTEGER NOT NULL,
                protected_payload TEXT NOT NULL
            );
            CREATE INDEX IF NOT EXISTS conversation_messages_conversation
                ON conversation_messages(conversation_id, id);
            CREATE TABLE IF NOT EXISTS document_arrivals (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                household_id TEXT NOT NULL,
                conversation_id INTEGER NOT NULL,
                protected_payload TEXT NOT NULL
            );
            CREATE INDEX IF NOT EXISTS document_arrivals_household
                ON document_arrivals(household_id);
            CREATE TABLE IF NOT EXISTS audit_events (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                household_id TEXT NOT NULL,
                arrival_id INTEGER NOT NULL,
                protected_payload TEXT NOT NULL
            );
            CREATE TABLE IF NOT EXISTS filing_rules (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                household_id TEXT NOT NULL,
                protected_payload TEXT NOT NULL
            );
            CREATE INDEX IF NOT EXISTS filing_rules_household
                ON filing_rules(household_id, id);
            CREATE TABLE IF NOT EXISTS portable_hidden_filing_rules (
                household_id TEXT NOT NULL,
                rule_id INTEGER NOT NULL,
                PRIMARY KEY (household_id, rule_id)
            );
            CREATE TABLE IF NOT EXISTS rebuilt_document_relationships (
                household_id TEXT NOT NULL,
                event_id TEXT NOT NULL,
                protected_payload TEXT NOT NULL,
                PRIMARY KEY (household_id, event_id)
            );
            CREATE TABLE IF NOT EXISTS rebuilt_portable_history (
                household_id TEXT NOT NULL,
                event_id TEXT NOT NULL,
                protected_payload TEXT NOT NULL,
                PRIMARY KEY (household_id, event_id)
            );
            CREATE TABLE IF NOT EXISTS filing_rule_events (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                household_id TEXT NOT NULL,
                rule_id INTEGER NOT NULL,
                protected_payload TEXT NOT NULL
            );
            CREATE INDEX IF NOT EXISTS filing_rule_events_household
                ON filing_rule_events(household_id, id);
            CREATE TABLE IF NOT EXISTS duplicate_preferences (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                household_id TEXT NOT NULL,
                protected_payload TEXT NOT NULL
            );
            CREATE INDEX IF NOT EXISTS duplicate_preferences_household
                ON duplicate_preferences(household_id, id);
            CREATE TABLE IF NOT EXISTS duplicate_events (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                household_id TEXT NOT NULL,
                arrival_id INTEGER NOT NULL,
                protected_payload TEXT NOT NULL
            );
            CREATE INDEX IF NOT EXISTS duplicate_events_household
                ON duplicate_events(household_id, id);
            CREATE TABLE IF NOT EXISTS household_work (
                id TEXT PRIMARY KEY,
                household_id TEXT NOT NULL,
                protected_payload TEXT NOT NULL
            );
            CREATE INDEX IF NOT EXISTS household_work_household
                ON household_work(household_id);
            CREATE INDEX IF NOT EXISTS audit_events_household
                ON audit_events(household_id, id);
            CREATE UNIQUE INDEX IF NOT EXISTS audit_events_arrival
                ON audit_events(arrival_id);",
        )?;
        Ok(store)
    }

    pub fn create_conversation(
        &self,
        household_id: &str,
        title: &str,
    ) -> Result<Conversation, ConversationError> {
        let title = title.trim();
        if title.is_empty() {
            return Err(ConversationError::EmptyTitle);
        }
        let payload = ConversationPayload {
            title: title.to_owned(),
            archived: false,
            deleted: false,
        };
        let protected = self.protect(household_id, &payload)?;
        let connection = self.connect()?;
        connection.execute(
            "INSERT INTO conversations (household_id, protected_payload) VALUES (?1, ?2)",
            params![household_id, protected],
        )?;
        Ok(Conversation {
            id: connection.last_insert_rowid(),
            household_id: household_id.to_owned(),
            title: title.to_owned(),
            archived: false,
        })
    }

    pub fn list_conversations(
        &self,
        household_id: &str,
        search: Option<&str>,
        include_archived: bool,
    ) -> Result<Vec<Conversation>, ConversationError> {
        let search = search.unwrap_or_default().trim().to_lowercase();
        let connection = self.connect()?;
        let mut statement = connection.prepare(
            "SELECT id, protected_payload FROM conversations
              WHERE household_id = ?1 ORDER BY id DESC",
        )?;
        let protected_rows = statement
            .query_map(params![household_id], |row| {
                Ok((row.get::<_, i64>(0)?, row.get::<_, String>(1)?))
            })?
            .collect::<Result<Vec<_>, _>>()?;
        let mut conversations = Vec::new();
        for (id, protected) in protected_rows {
            let payload: ConversationPayload = self.open_protected(household_id, &protected)?;
            if payload.deleted || (!include_archived && payload.archived) {
                continue;
            }
            if !payload.title.to_lowercase().contains(&search) {
                continue;
            }
            conversations.push(Conversation {
                id,
                household_id: household_id.to_owned(),
                title: payload.title,
                archived: payload.archived,
            });
        }
        Ok(conversations)
    }

    pub fn rename_conversation(
        &self,
        household_id: &str,
        conversation_id: i64,
        title: &str,
    ) -> Result<(), ConversationError> {
        let title = title.trim();
        if title.is_empty() {
            return Err(ConversationError::EmptyTitle);
        }
        let mut payload = self.load_conversation_payload(household_id, conversation_id)?;
        if payload.deleted {
            return Err(ConversationError::NotFound);
        }
        payload.title = title.to_owned();
        self.save_conversation_payload(household_id, conversation_id, &payload)
    }

    pub fn archive_conversation(
        &self,
        household_id: &str,
        conversation_id: i64,
        archived: bool,
    ) -> Result<(), ConversationError> {
        let mut payload = self.load_conversation_payload(household_id, conversation_id)?;
        if payload.deleted {
            return Err(ConversationError::NotFound);
        }
        payload.archived = archived;
        self.save_conversation_payload(household_id, conversation_id, &payload)
    }

    pub fn delete_conversation(
        &self,
        household_id: &str,
        conversation_id: i64,
    ) -> Result<(), ConversationError> {
        let mut payload = self.load_conversation_payload(household_id, conversation_id)?;
        if payload.deleted {
            return Err(ConversationError::NotFound);
        }
        payload.title = "Deleted Conversation".to_owned();
        payload.archived = false;
        payload.deleted = true;
        let protected = self.protect(household_id, &payload)?;
        let mut connection = self.connect()?;
        let transaction = connection.transaction()?;
        transaction.execute(
            "UPDATE conversations SET protected_payload = ?1
              WHERE id = ?2 AND household_id = ?3",
            params![protected, conversation_id, household_id],
        )?;
        transaction.execute(
            "DELETE FROM conversation_messages WHERE conversation_id = ?1",
            params![conversation_id],
        )?;
        transaction.commit()?;
        Ok(())
    }

    pub fn attach_document(
        &self,
        household_id: &str,
        conversation_id: i64,
        path: impl AsRef<Path>,
        cabinet_root: impl AsRef<Path>,
    ) -> Result<DocumentArrival, ConversationError> {
        self.require_active_conversation(household_id, conversation_id)?;
        let path = path.as_ref();
        if !path.is_file() {
            return Err(ConversationError::DocumentUnavailable(io::Error::new(
                io::ErrorKind::NotFound,
                "document is not a file",
            )));
        }
        let declared_media_type = match path
            .extension()
            .and_then(|extension| extension.to_str())
            .map(str::to_ascii_lowercase)
            .as_deref()
        {
            Some("pdf") => "application/pdf",
            Some("jpg" | "jpeg") => "image/jpeg",
            Some("png") => "image/png",
            _ => return Err(ConversationError::UnsupportedDocument),
        };
        if fs::metadata(path)?.len() > MAX_MVP_DOCUMENT_BYTES {
            return Err(ConversationError::DocumentTooLarge);
        }
        let mut original = Vec::new();
        fs::File::open(path)?
            .take(MAX_MVP_DOCUMENT_BYTES + 1)
            .read_to_end(&mut original)?;
        if original.len() as u64 > MAX_MVP_DOCUMENT_BYTES {
            return Err(ConversationError::DocumentTooLarge);
        }
        let media_type = detected_media_type(&original)?;
        if media_type != declared_media_type {
            return Err(ConversationError::InvalidDocument);
        }
        let original_name = path
            .file_name()
            .and_then(|name| name.to_str())
            .ok_or(ConversationError::UnsupportedDocument)?;
        let extracted_pdf_text = extract_digital_pdf_text(media_type, &original)?;
        let checksum = sha256(&original);
        let original_path =
            self.preserve_original(cabinet_root.as_ref(), &checksum, original_name, &original)?;
        let extracted_text = extract_local_text(
            media_type,
            &original_path,
            extracted_pdf_text,
            &*self.local_ocr,
        );
        let context_direction = local_context_direction(extracted_text.as_deref());
        let processing_state = if local_evidence_needs_direction_interpretation(
            extracted_text.as_deref(),
            &context_direction,
        ) {
            DocumentProcessingState::NeedsCloudConsent
        } else {
            DocumentProcessingState::NeedsMemberDirection
        };
        let mut payload = DocumentArrivalPayload {
            original_name: original_name.to_owned(),
            original_path,
            source_path: path.to_owned(),
            checksum,
            media_type: media_type.to_owned(),
            extracted_text,
            processing_state,
            context_direction,
            filing_decision: None,
            learned_rule: None,
            automatic_rule_id: None,
            filed_original: None,
            duplicate_review: None,
            duplicate_resolution: None,
            cloud_assistance_history: Vec::new(),
            execution_history: vec![
                "Verified Original staged and inspected locally; no filing action completed."
                    .to_owned(),
            ],
            filing_decision_declined: false,
            filing_rule_declined: false,
        };
        let arrival_id = {
            let protected = self.protect(household_id, &payload)?;
            let connection = self.connect()?;
            connection.execute(
                "INSERT INTO document_arrivals (household_id, conversation_id, protected_payload)
                 VALUES (?1, ?2, ?3)",
                params![household_id, conversation_id, protected],
            )?;
            connection.last_insert_rowid()
        };
        if let Some(duplicate_review) =
            self.find_duplicate_review(household_id, arrival_id, &payload)?
        {
            payload.processing_state = DocumentProcessingState::PossibleDuplicate;
            payload.duplicate_review = Some(duplicate_review);
            self.save_document_arrival_payload(
                household_id,
                arrival_id,
                conversation_id,
                payload.clone(),
            )?;
            if let Some(preference) = self.matching_duplicate_preference(household_id, &payload)? {
                return self.resolve_duplicate_internal(
                    household_id,
                    arrival_id,
                    preference.related_arrival_id,
                    preference.decision,
                    false,
                    DuplicateAuditKind::DuplicatePreferenceApplied,
                );
            }
            return self.document_arrival(household_id, arrival_id, conversation_id, payload);
        }
        if let Some(rule) = self.matching_filing_rule(
            household_id,
            &payload.context_direction,
            payload.extracted_text.as_deref(),
        )? {
            payload.processing_state = DocumentProcessingState::ReadyToFile;
            payload.filing_decision = Some(automatic_filing_decision(&payload, &rule));
            payload.learned_rule = Some(rule.clone());
            payload.automatic_rule_id = Some(rule.id);
            push_history(
                &mut payload.execution_history,
                "A scoped Filing Rule matched and authorised filing.",
            );
            let protected = self.protect(household_id, &payload)?;
            self.ensure_updated(
                "UPDATE document_arrivals SET protected_payload = ?1 WHERE id = ?2 AND household_id = ?3",
                params![protected, arrival_id, household_id],
            )?;
            return self.file_document(household_id, arrival_id, cabinet_root);
        }
        self.document_arrival(household_id, arrival_id, conversation_id, payload)
    }

    pub fn add_member_message(
        &self,
        household_id: &str,
        conversation_id: i64,
        body: &str,
    ) -> Result<ConversationMessage, ConversationError> {
        self.add_conversation_message(household_id, conversation_id, "member", body, None)
    }

    pub fn add_luna_message(
        &self,
        household_id: &str,
        conversation_id: i64,
        body: &str,
        linked_document_arrival: Option<i64>,
    ) -> Result<ConversationMessage, ConversationError> {
        self.add_conversation_message(
            household_id,
            conversation_id,
            "luna",
            body,
            linked_document_arrival,
        )
    }

    fn add_conversation_message(
        &self,
        household_id: &str,
        conversation_id: i64,
        author: &str,
        body: &str,
        linked_document_arrival: Option<i64>,
    ) -> Result<ConversationMessage, ConversationError> {
        self.require_active_conversation(household_id, conversation_id)?;
        let body = body.trim();
        if body.is_empty() {
            return Err(ConversationError::EmptyMessage);
        }
        let payload = MessagePayload {
            author: author.to_owned(),
            body: body.to_owned(),
            linked_document_arrival,
        };
        let protected = self.protect(household_id, &payload)?;
        let connection = self.connect()?;
        connection.execute(
            "INSERT INTO conversation_messages (conversation_id, protected_payload)
             VALUES (?1, ?2)",
            params![conversation_id, protected],
        )?;
        Ok(ConversationMessage {
            id: connection.last_insert_rowid(),
            conversation_id,
            author: payload.author,
            body: payload.body,
            linked_document_arrival: payload.linked_document_arrival,
        })
    }

    pub fn list_messages(
        &self,
        household_id: &str,
        conversation_id: i64,
    ) -> Result<Vec<ConversationMessage>, ConversationError> {
        let conversation = self.load_conversation_payload(household_id, conversation_id)?;
        if conversation.deleted {
            return Ok(Vec::new());
        }
        let connection = self.connect()?;
        let mut statement = connection.prepare(
            "SELECT id, protected_payload FROM conversation_messages
              WHERE conversation_id = ?1 ORDER BY id",
        )?;
        let protected_rows = statement
            .query_map(params![conversation_id], |row| {
                Ok((row.get::<_, i64>(0)?, row.get::<_, String>(1)?))
            })?
            .collect::<Result<Vec<_>, _>>()?;
        protected_rows
            .into_iter()
            .map(|(id, protected)| {
                let payload: MessagePayload = self.open_protected(household_id, &protected)?;
                Ok(ConversationMessage {
                    id,
                    conversation_id,
                    author: payload.author,
                    body: payload.body,
                    linked_document_arrival: payload.linked_document_arrival,
                })
            })
            .collect()
    }

    pub fn list_household_work(
        &self,
        household_id: &str,
    ) -> Result<Vec<HouseholdWork>, ConversationError> {
        let connection = self.connect()?;
        let mut statement = connection.prepare(
            "SELECT id, protected_payload FROM household_work
             WHERE household_id = ?1 ORDER BY rowid DESC",
        )?;
        let rows = statement
            .query_map(params![household_id], |row| {
                Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
            })?
            .collect::<Result<Vec<_>, _>>()?;
        rows.into_iter()
            .map(|(id, protected)| {
                let mut work: HouseholdWork = self.open_protected(household_id, &protected)?;
                work.id = id;
                Ok(work)
            })
            .collect()
    }

    pub fn save_household_work(
        &self,
        household_id: &str,
        work: &HouseholdWork,
    ) -> Result<(), ConversationError> {
        if work.household_id != household_id || work.id.trim().is_empty() {
            return Err(ConversationError::NotFound);
        }
        let protected = self.protect(household_id, work)?;
        self.connect()?.execute(
            "INSERT INTO household_work (id, household_id, protected_payload)
             VALUES (?1, ?2, ?3)
             ON CONFLICT(id) DO UPDATE SET
               household_id = excluded.household_id,
               protected_payload = excluded.protected_payload",
            params![work.id, household_id, protected],
        )?;
        Ok(())
    }

    pub fn find_household_work(
        &self,
        household_id: &str,
        work_id: &str,
    ) -> Result<HouseholdWork, ConversationError> {
        self.list_household_work(household_id)?
            .into_iter()
            .find(|work| work.id == work_id)
            .ok_or(ConversationError::NotFound)
    }

    pub fn active_household_work_for_source(
        &self,
        household_id: &str,
        source_ref: &str,
    ) -> Result<Option<HouseholdWork>, ConversationError> {
        Ok(self
            .list_household_work(household_id)?
            .into_iter()
            .find(|work| {
                work.status.is_open() && work.source_refs.iter().any(|item| item == source_ref)
            }))
    }

    pub fn household_work_for_source(
        &self,
        household_id: &str,
        source_ref: &str,
    ) -> Result<Option<HouseholdWork>, ConversationError> {
        Ok(self
            .list_household_work(household_id)?
            .into_iter()
            .find(|work| work.source_refs.iter().any(|item| item == source_ref)))
    }

    pub fn apply_household_administration_result(
        &self,
        household_id: &str,
        conversation_id: i64,
        arrival_id: Option<i64>,
        result: &HouseholdAdministrationResult,
        now: &str,
    ) -> Result<Option<HouseholdWork>, ConversationError> {
        let source_ref = arrival_id.map(|id| format!("document-{id}"));
        let existing = result
            .work
            .work_id
            .as_deref()
            .map(|id| self.find_household_work(household_id, id))
            .transpose()?
            .or_else(|| {
                source_ref.as_deref().and_then(|source| {
                    self.household_work_for_source(household_id, source)
                        .ok()
                        .flatten()
                })
            });
        let work = apply_result(
            household_id,
            conversation_id,
            source_ref.as_deref(),
            existing,
            result,
            now,
        )
        .map_err(|failure| match failure.category {
            HouseholdAdministrationFailureCategory::MissingExistingWork => {
                ConversationError::NotFound
            }
            _ => ConversationError::InvalidHouseholdWorkTransition,
        })?;
        if matches!(result.work.operation, crate::HouseholdWorkOperation::None) {
            return Ok(work);
        }
        if let Some(work) = work.as_ref() {
            self.save_household_work(household_id, work)?;
        }
        Ok(work)
    }

    pub fn list_document_arrivals(
        &self,
        household_id: &str,
    ) -> Result<Vec<DocumentArrival>, ConversationError> {
        let connection = self.connect()?;
        let mut statement = connection.prepare(
            "SELECT id, conversation_id, protected_payload
               FROM document_arrivals WHERE household_id = ?1 ORDER BY id DESC",
        )?;
        let protected_rows = statement
            .query_map(params![household_id], |row| {
                Ok((
                    row.get::<_, i64>(0)?,
                    row.get::<_, i64>(1)?,
                    row.get::<_, String>(2)?,
                ))
            })?
            .collect::<Result<Vec<_>, _>>()?;
        protected_rows
            .into_iter()
            .map(|(id, conversation_id, protected)| {
                let mut payload: DocumentArrivalPayload =
                    self.open_protected(household_id, &protected)?;
                payload.restore_legacy_original_path();
                self.document_arrival(household_id, id, conversation_id, payload)
            })
            .collect()
    }

    pub fn begin_cloud_assistance(
        &self,
        household_id: &str,
        arrival_id: i64,
    ) -> Result<DocumentArrival, ConversationError> {
        self.transition_cloud_assistance(
            household_id,
            arrival_id,
            &[
                DocumentProcessingState::NeedsCloudConsent,
                DocumentProcessingState::WaitingForCloudAssistance,
                DocumentProcessingState::NeedsMemberDirection,
            ],
            DocumentProcessingState::InspectingWithAssistance,
        )
    }

    pub fn keep_document_local(
        &self,
        household_id: &str,
        arrival_id: i64,
    ) -> Result<DocumentArrival, ConversationError> {
        self.transition_cloud_assistance(
            household_id,
            arrival_id,
            &[
                DocumentProcessingState::NeedsCloudConsent,
                DocumentProcessingState::WaitingForCloudAssistance,
                DocumentProcessingState::InspectingWithAssistance,
            ],
            DocumentProcessingState::NeedsMemberDirection,
        )
    }

    pub fn complete_cloud_assistance(
        &self,
        household_id: &str,
        arrival_id: i64,
    ) -> Result<DocumentArrival, ConversationError> {
        self.transition_cloud_assistance(
            household_id,
            arrival_id,
            &[DocumentProcessingState::InspectingWithAssistance],
            DocumentProcessingState::NeedsMemberDirection,
        )
    }

    pub fn wait_for_cloud_assistance(
        &self,
        household_id: &str,
        arrival_id: i64,
    ) -> Result<DocumentArrival, ConversationError> {
        self.transition_cloud_assistance(
            household_id,
            arrival_id,
            &[DocumentProcessingState::InspectingWithAssistance],
            DocumentProcessingState::WaitingForCloudAssistance,
        )
    }

    pub fn validate_candidate_direction(
        &self,
        household_id: &str,
        arrival_id: i64,
        candidate: CandidateDirectionInterpretation,
    ) -> Result<CandidateDirectionInterpretation, ConversationError> {
        let (_, payload) = self.load_document_arrival_payload(household_id, arrival_id)?;
        if payload.processing_state != DocumentProcessingState::InspectingWithAssistance {
            return Err(ConversationError::InvalidDirectionInterpretation);
        }
        let unresolved_fields = unresolved_candidate_fields(&payload.context_direction);
        let candidate_fields = [
            (
                ContextField::DocumentType,
                candidate.document_type.is_some(),
            ),
            (
                ContextField::ServiceProvider,
                candidate.service_provider.is_some(),
            ),
            (ContextField::Addressee, candidate.addressee.is_some()),
            (ContextField::Property, candidate.property.is_some()),
            (ContextField::Account, candidate.account.is_some()),
            (ContextField::Amount, candidate.amount.is_some()),
            (
                ContextField::RelevantDates,
                !candidate.relevant_dates.is_empty(),
            ),
        ];
        if candidate_fields.iter().any(|(field, supplied)| {
            *supplied
                && !unresolved_fields
                    .iter()
                    .any(|unresolved| unresolved == field)
        }) {
            return Err(ConversationError::InvalidDirectionInterpretation);
        }
        Ok(CandidateDirectionInterpretation {
            document_type: valid_candidate_value(candidate.document_type)?,
            service_provider: valid_candidate_value(candidate.service_provider)?,
            addressee: valid_candidate_value(candidate.addressee)?,
            property: valid_candidate_value(candidate.property)?,
            account: valid_candidate_value(candidate.account)?,
            amount: valid_candidate_amount(candidate.amount)?,
            relevant_dates: candidate
                .relevant_dates
                .into_iter()
                .map(valid_candidate_date)
                .collect::<Result<Vec<_>, _>>()?
                .into_iter()
                .flatten()
                .collect(),
        })
    }

    pub fn list_todo_items(&self, household_id: &str) -> Result<Vec<TodoItem>, ConversationError> {
        let arrivals = self.list_document_arrivals(household_id)?;
        let household_work = self.list_household_work(household_id)?;
        arrivals
            .into_iter()
            .filter(|arrival| {
                let source_ref = format!("document-{}", arrival.id);
                let linked_work = household_work
                    .iter()
                    .filter(|work| work.source_refs.iter().any(|source| source == &source_ref))
                    .collect::<Vec<_>>();
                if !linked_work.is_empty() {
                    return linked_work.iter().any(|work| work.status.is_open());
                }
                // Transitional compatibility: arrivals without Household Work still use the
                // legacy Document Handling projection until that migration is complete.
                matches!(
                    arrival.processing_state,
                    DocumentProcessingState::NeedsCloudConsent
                        | DocumentProcessingState::InspectingWithAssistance
                        | DocumentProcessingState::WaitingForCloudAssistance
                        | DocumentProcessingState::NeedsMemberDirection
                        | DocumentProcessingState::PossibleDuplicate
                        | DocumentProcessingState::CabinetUnavailable
                )
            })
            .map(|arrival| {
                let conversation =
                    self.load_conversation_payload(household_id, arrival.conversation_id)?;
                Ok(TodoItem {
                    arrival_id: arrival.id,
                    conversation_id: arrival.conversation_id,
                    conversation_title: conversation.title,
                    conversation_deleted: conversation.deleted,
                    document_name: arrival.original_name,
                    processing_state: arrival.processing_state,
                })
            })
            .collect()
    }

    pub fn list_filed_originals(
        &self,
        household_id: &str,
    ) -> Result<Vec<FiledOriginal>, ConversationError> {
        Ok(self
            .list_document_arrivals(household_id)?
            .into_iter()
            .filter_map(|arrival| arrival.filed_original)
            .collect())
    }

    pub fn list_audit_events(
        &self,
        household_id: &str,
    ) -> Result<Vec<AuditEvent>, ConversationError> {
        let connection = self.connect()?;
        let mut statement = connection.prepare(
            "SELECT id, protected_payload FROM audit_events
              WHERE household_id = ?1 ORDER BY id DESC",
        )?;
        let protected_rows = statement
            .query_map(params![household_id], |row| {
                Ok((row.get::<_, i64>(0)?, row.get::<_, String>(1)?))
            })?
            .collect::<Result<Vec<_>, _>>()?;
        protected_rows
            .into_iter()
            .map(|(id, protected)| {
                let payload: AuditEventPayload = self.open_protected(household_id, &protected)?;
                Ok(AuditEvent {
                    id,
                    household_id: household_id.to_owned(),
                    kind: payload.kind,
                    authority: payload.authority,
                    subject: payload.subject,
                    outcome: payload.outcome,
                    filed_original: payload.filed_original,
                })
            })
            .collect()
    }

    pub fn list_duplicate_audit_events(
        &self,
        household_id: &str,
    ) -> Result<Vec<DuplicateAuditEvent>, ConversationError> {
        let connection = self.connect()?;
        let mut statement = connection.prepare(
            "SELECT id, protected_payload FROM duplicate_events
              WHERE household_id = ?1 ORDER BY id DESC",
        )?;
        let protected_rows = statement
            .query_map(params![household_id], |row| {
                Ok((row.get::<_, i64>(0)?, row.get::<_, String>(1)?))
            })?
            .collect::<Result<Vec<_>, _>>()?;
        protected_rows
            .into_iter()
            .map(|(id, protected)| {
                let payload: DuplicateAuditPayload =
                    self.open_protected(household_id, &protected)?;
                Ok(DuplicateAuditEvent {
                    id,
                    household_id: household_id.to_owned(),
                    kind: payload.kind,
                    decision: payload.decision,
                    subject: payload.subject,
                    outcome: payload.outcome,
                    related_arrival_id: payload.related_arrival_id,
                })
            })
            .collect()
    }

    pub fn resolve_duplicate(
        &self,
        household_id: &str,
        arrival_id: i64,
        related_arrival_id: i64,
        decision: DuplicateDecision,
        remember_preference: bool,
    ) -> Result<DocumentArrival, ConversationError> {
        self.resolve_duplicate_internal(
            household_id,
            arrival_id,
            related_arrival_id,
            decision,
            remember_preference,
            DuplicateAuditKind::DuplicateDecisionRecorded,
        )
    }

    pub fn list_filing_rules(
        &self,
        household_id: &str,
    ) -> Result<Vec<FilingRuleSummary>, ConversationError> {
        let connection = self.connect()?;
        let mut statement = connection.prepare(
            "SELECT id, protected_payload FROM filing_rules
              WHERE household_id = ?1
                AND NOT EXISTS (
                    SELECT 1 FROM portable_hidden_filing_rules AS hidden
                     WHERE hidden.household_id = filing_rules.household_id
                       AND hidden.rule_id = filing_rules.id
                )
              ORDER BY id DESC",
        )?;
        let protected_rows = statement
            .query_map(params![household_id], |row| {
                Ok((row.get::<_, i64>(0)?, row.get::<_, String>(1)?))
            })?
            .collect::<Result<Vec<_>, _>>()?;
        protected_rows
            .into_iter()
            .map(|(id, protected)| {
                let mut rule: FilingRule = self.open_protected(household_id, &protected)?;
                rule.id = id;
                self.filing_rule_summary(household_id, rule)
            })
            .collect()
    }

    pub(crate) fn apply_portable_filing_rule(
        &self,
        household_id: &str,
        local_id: Option<i64>,
        definition: &PortableFilingRuleDefinition,
        state: &PortableFilingRuleState,
    ) -> Result<i64, ConversationError> {
        let mut rule = FilingRule {
            id: local_id.unwrap_or_default(),
            document_type: definition.document_type.clone(),
            service_provider: definition.service_provider.clone(),
            addressee: definition.addressee.clone(),
            property: definition.property.clone(),
            account: definition.account.clone(),
            file_name: definition.file_name.clone(),
            cabinet_destination: definition.cabinet_destination.clone(),
            teacher: definition.taught_by.to_string(),
            created_at: definition.created_at.clone(),
            paused: matches!(
                state,
                PortableFilingRuleState::Paused | PortableFilingRuleState::Deleted
            ),
            deleted: *state == PortableFilingRuleState::Deleted,
        };
        let protected = self.protect(household_id, &rule)?;
        let connection = self.connect()?;
        if let Some(local_id) = local_id {
            let updated = connection.execute(
                "UPDATE filing_rules SET protected_payload = ?1
                  WHERE household_id = ?2 AND id = ?3",
                params![protected, household_id, local_id],
            )?;
            if updated > 0 {
                return Ok(local_id);
            }
        }
        connection.execute(
            "INSERT INTO filing_rules (household_id, protected_payload) VALUES (?1, ?2)",
            params![household_id, protected],
        )?;
        let local_id = connection.last_insert_rowid();
        rule.id = local_id;
        connection.execute(
            "UPDATE filing_rules SET protected_payload = ?1
              WHERE household_id = ?2 AND id = ?3",
            params![self.protect(household_id, &rule)?, household_id, local_id],
        )?;
        Ok(local_id)
    }

    pub(crate) fn set_portable_filing_rule_visibility(
        &self,
        household_id: &str,
        all_bound_rule_ids: &[i64],
        visible_rule_ids: &[i64],
    ) -> Result<(), ConversationError> {
        let mut connection = self.connect()?;
        let transaction = connection.transaction()?;
        for rule_id in all_bound_rule_ids {
            if visible_rule_ids.contains(rule_id) {
                transaction.execute(
                    "DELETE FROM portable_hidden_filing_rules
                      WHERE household_id = ?1 AND rule_id = ?2",
                    params![household_id, rule_id],
                )?;
            } else {
                transaction.execute(
                    "INSERT OR IGNORE INTO portable_hidden_filing_rules (
                        household_id, rule_id
                     ) VALUES (?1, ?2)",
                    params![household_id, rule_id],
                )?;
            }
        }
        transaction.commit()?;
        Ok(())
    }

    pub(crate) fn replace_rebuilt_document_relationships(
        &self,
        household_id: &str,
        relationships: &[RebuiltDocumentRelationship],
    ) -> Result<(), ConversationError> {
        let mut connection = self.connect()?;
        let transaction = connection.transaction()?;
        transaction.execute(
            "DELETE FROM rebuilt_document_relationships WHERE household_id = ?1",
            params![household_id],
        )?;
        for relationship in relationships {
            transaction.execute(
                "INSERT INTO rebuilt_document_relationships (
                    household_id, event_id, protected_payload
                 ) VALUES (?1, ?2, ?3)",
                params![
                    household_id,
                    relationship.event_id,
                    self.protect(household_id, relationship)?
                ],
            )?;
        }
        transaction.commit()?;
        Ok(())
    }

    pub fn list_rebuilt_document_relationships(
        &self,
        household_id: &str,
    ) -> Result<Vec<RebuiltDocumentRelationship>, ConversationError> {
        let connection = self.connect()?;
        let mut statement = connection.prepare(
            "SELECT protected_payload FROM rebuilt_document_relationships
              WHERE household_id = ?1 ORDER BY event_id",
        )?;
        let protected = statement
            .query_map(params![household_id], |row| row.get::<_, String>(0))?
            .collect::<Result<Vec<_>, _>>()?;
        protected
            .into_iter()
            .map(|payload| self.open_protected(household_id, &payload))
            .collect()
    }

    pub(crate) fn replace_rebuilt_portable_history(
        &self,
        household_id: &str,
        history: &[PortableHistoryEvent],
    ) -> Result<(), ConversationError> {
        let mut connection = self.connect()?;
        let transaction = connection.transaction()?;
        transaction.execute(
            "DELETE FROM rebuilt_portable_history WHERE household_id = ?1",
            params![household_id],
        )?;
        for event in history {
            transaction.execute(
                "INSERT INTO rebuilt_portable_history (
                    household_id, event_id, protected_payload
                 ) VALUES (?1, ?2, ?3)",
                params![
                    household_id,
                    event.event_id,
                    self.protect(household_id, event)?
                ],
            )?;
        }
        transaction.commit()?;
        Ok(())
    }

    pub fn list_rebuilt_portable_history(
        &self,
        household_id: &str,
    ) -> Result<Vec<PortableHistoryEvent>, ConversationError> {
        let connection = self.connect()?;
        let mut statement = connection.prepare(
            "SELECT protected_payload FROM rebuilt_portable_history
              WHERE household_id = ?1 ORDER BY event_id",
        )?;
        let protected = statement
            .query_map(params![household_id], |row| row.get::<_, String>(0))?
            .collect::<Result<Vec<_>, _>>()?;
        protected
            .into_iter()
            .map(|payload| self.open_protected(household_id, &payload))
            .collect()
    }

    pub fn update_filing_rule(
        &self,
        household_id: &str,
        rule_id: i64,
        update: FilingRuleUpdate,
    ) -> Result<FilingRuleSummary, ConversationError> {
        let (mut rule, _) = self.load_filing_rule(household_id, rule_id)?;
        let update = update.normalized();
        if update.document_type.is_empty()
            || update.service_provider.is_empty()
            || update.addressee.is_empty()
            || (update.property.is_none() && update.account.is_none())
            || !valid_file_name(&update.file_name, &rule.file_name)
            || !valid_cabinet_destination(&update.cabinet_destination, &update.file_name)
        {
            return Err(ConversationError::InvalidCabinetDestination);
        }
        rule.document_type = update.document_type;
        rule.service_provider = update.service_provider;
        rule.addressee = update.addressee;
        rule.property = update.property;
        rule.account = update.account;
        rule.file_name = update.file_name;
        rule.cabinet_destination = update.cabinet_destination;
        self.save_filing_rule(
            household_id,
            rule_id,
            &rule,
            FilingRuleAuditKind::Updated,
            "updated prospectively",
        )?;
        self.filing_rule_summary(household_id, rule)
    }

    pub fn pause_filing_rule(
        &self,
        household_id: &str,
        rule_id: i64,
        paused: bool,
    ) -> Result<FilingRuleSummary, ConversationError> {
        let (mut rule, _) = self.load_filing_rule(household_id, rule_id)?;
        if rule.deleted {
            return Err(ConversationError::NotFound);
        }
        rule.paused = paused;
        self.save_filing_rule(
            household_id,
            rule_id,
            &rule,
            if paused {
                FilingRuleAuditKind::Paused
            } else {
                FilingRuleAuditKind::Resumed
            },
            if paused { "paused" } else { "resumed" },
        )?;
        self.filing_rule_summary(household_id, rule)
    }

    pub fn delete_filing_rule(
        &self,
        household_id: &str,
        rule_id: i64,
    ) -> Result<FilingRuleSummary, ConversationError> {
        let (mut rule, _) = self.load_filing_rule(household_id, rule_id)?;
        if rule.deleted {
            return Err(ConversationError::NotFound);
        }
        rule.deleted = true;
        rule.paused = true;
        self.save_filing_rule(
            household_id,
            rule_id,
            &rule,
            FilingRuleAuditKind::Deleted,
            "deleted for future arrivals; historical Originals were left unchanged",
        )?;
        self.filing_rule_summary(household_id, rule)
    }

    pub fn list_filing_rule_audit_events(
        &self,
        household_id: &str,
    ) -> Result<Vec<FilingRuleAuditEvent>, ConversationError> {
        let connection = self.connect()?;
        let mut statement = connection.prepare(
            "SELECT id, rule_id, protected_payload FROM filing_rule_events
              WHERE household_id = ?1 ORDER BY id DESC",
        )?;
        let protected_rows = statement
            .query_map(params![household_id], |row| {
                Ok((
                    row.get::<_, i64>(0)?,
                    row.get::<_, i64>(1)?,
                    row.get::<_, String>(2)?,
                ))
            })?
            .collect::<Result<Vec<_>, _>>()?;
        protected_rows
            .into_iter()
            .map(|(id, rule_id, protected)| {
                let payload: FilingRuleAuditPayload =
                    self.open_protected(household_id, &protected)?;
                Ok(FilingRuleAuditEvent {
                    id,
                    household_id: household_id.to_owned(),
                    rule_id,
                    kind: payload.kind,
                    subject: payload.subject,
                    outcome: payload.outcome,
                })
            })
            .collect()
    }

    pub fn preview_filing_rule_reorganization(
        &self,
        household_id: &str,
        rule_id: i64,
        proposed_directory: &str,
    ) -> Result<FilingRuleReorganizationPreview, ConversationError> {
        let (rule, _) = self.load_filing_rule(household_id, rule_id)?;
        let proposed_directory = proposed_directory.trim().trim_matches('/');
        if !valid_rule_directory(proposed_directory) {
            return Err(ConversationError::InvalidCabinetDestination);
        }
        let documents = self
            .list_document_arrivals(household_id)?
            .into_iter()
            .filter_map(|arrival| {
                let learned = arrival.review_card.learned_rule.as_ref()?;
                if learned.id != rule.id {
                    return None;
                }
                let filed = arrival.filed_original?;
                let file_name = filed.filing_decision.file_name;
                Some(FilingRuleReorganizationDocument {
                    arrival_id: arrival.id,
                    original_name: arrival.original_name,
                    current_destination: filed.filing_decision.cabinet_destination,
                    proposed_destination: format!("{proposed_directory}/{file_name}"),
                })
            })
            .collect();
        Ok(FilingRuleReorganizationPreview {
            rule_id,
            proposed_directory: proposed_directory.to_owned(),
            documents,
        })
    }

    pub fn list_manual_move_candidates(
        &self,
        household_id: &str,
        cabinet_root: impl AsRef<Path>,
    ) -> Result<Vec<ManualMoveCandidate>, ConversationError> {
        let cabinet_root = cabinet_root.as_ref();
        self.list_document_arrivals(household_id)?
            .into_iter()
            .filter_map(|arrival| {
                let filed = arrival.filed_original?;
                if filed.final_path.exists() {
                    return None;
                }
                let current_path = find_checksum_file(cabinet_root, &filed.checksum)?;
                let relative = current_path.strip_prefix(cabinet_root).ok()?;
                let current_destination = relative
                    .iter()
                    .map(|part| part.to_string_lossy())
                    .collect::<Vec<_>>()
                    .join("/");
                Some(Ok(ManualMoveCandidate {
                    arrival_id: arrival.id,
                    original_name: arrival.original_name,
                    previous_destination: filed.filing_decision.cabinet_destination,
                    current_destination,
                }))
            })
            .collect()
    }

    pub fn record_manual_move_decision(
        &self,
        household_id: &str,
        arrival_id: i64,
        cabinet_root: impl AsRef<Path>,
        teaches_rule: bool,
    ) -> Result<DocumentArrival, ConversationError> {
        let candidate = self
            .list_manual_move_candidates(household_id, cabinet_root.as_ref())?
            .into_iter()
            .find(|candidate| candidate.arrival_id == arrival_id)
            .ok_or(ConversationError::NotFound)?;
        let (conversation_id, mut payload) =
            self.load_document_arrival_payload(household_id, arrival_id)?;
        let filed = payload
            .filed_original
            .as_mut()
            .ok_or(ConversationError::NotFound)?;
        filed.final_path = cabinet_root.as_ref().join(&candidate.current_destination);
        filed.filing_decision.file_name = Path::new(&candidate.current_destination)
            .file_name()
            .and_then(|name| name.to_str())
            .ok_or(ConversationError::InvalidCabinetDestination)?
            .to_owned();
        filed.filing_decision.cabinet_destination = candidate.current_destination.clone();
        let rule_id = payload
            .learned_rule
            .as_ref()
            .map(|rule| rule.id)
            .ok_or(ConversationError::NotFound)?;
        let saved =
            self.save_document_arrival_payload(household_id, arrival_id, conversation_id, payload)?;
        let (mut rule, _) = self.load_filing_rule(household_id, rule_id)?;
        if teaches_rule {
            rule.file_name = saved
                .review_card
                .filing_decision
                .as_ref()
                .ok_or(ConversationError::NotFound)?
                .file_name
                .clone();
            rule.cabinet_destination = candidate.current_destination.clone();
            self.save_filing_rule(
                household_id,
                rule_id,
                &rule,
                FilingRuleAuditKind::Updated,
                "learned from the owner's manual Cabinet move",
            )?;
        } else {
            self.insert_filing_rule_event(
                household_id,
                rule_id,
                &rule,
                FilingRuleAuditKind::Updated,
                "kept as a one-off exception; Luna will not reverse the owner's move",
            )?;
        }
        Ok(saved)
    }

    pub fn dismiss_document_arrival(
        &self,
        household_id: &str,
        arrival_id: i64,
    ) -> Result<(), ConversationError> {
        let connection = self.connect()?;
        let protected: Option<String> = connection
            .query_row(
                "SELECT protected_payload FROM document_arrivals
                  WHERE id = ?1 AND household_id = ?2",
                params![arrival_id, household_id],
                |row| row.get(0),
            )
            .optional()?;
        let mut payload: DocumentArrivalPayload =
            self.open_protected(household_id, &protected.ok_or(ConversationError::NotFound)?)?;
        payload.restore_legacy_original_path();
        if !matches!(
            payload.processing_state,
            DocumentProcessingState::NeedsCloudConsent
                | DocumentProcessingState::WaitingForCloudAssistance
                | DocumentProcessingState::NeedsMemberDirection
        ) {
            return Err(ConversationError::NotFound);
        }
        payload.processing_state = DocumentProcessingState::Dismissed;
        let protected = self.protect(household_id, &payload)?;
        self.ensure_updated(
            "UPDATE document_arrivals SET protected_payload = ?1
              WHERE id = ?2 AND household_id = ?3",
            params![protected, arrival_id, household_id],
        )
    }

    pub fn document_conversation(
        &self,
        household_id: &str,
        arrival_id: i64,
    ) -> Result<DocumentConversationView, ConversationError> {
        self.document_conversation_in_section(household_id, arrival_id, "Household records")
    }

    pub fn document_conversation_in_section(
        &self,
        household_id: &str,
        arrival_id: i64,
        cabinet_section: &str,
    ) -> Result<DocumentConversationView, ConversationError> {
        let (conversation_id, payload) =
            self.load_document_arrival_payload(household_id, arrival_id)?;
        let arrival = self.document_arrival(household_id, arrival_id, conversation_id, payload)?;
        Ok(document_conversation_view(&arrival, cabinet_section))
    }

    pub fn submit_member_utterance(
        &self,
        household_id: &str,
        arrival_id: i64,
        utterance: MemberUtterance,
        interpreter: &dyn MemberDirectionInterpreter,
        cabinet_root: impl AsRef<Path>,
        cabinet_section: &str,
    ) -> Result<ConversationTurnOutcome, ConversationError> {
        let (conversation_id, payload) =
            self.load_document_arrival_payload(household_id, arrival_id)?;
        let arrival = self.document_arrival(household_id, arrival_id, conversation_id, payload)?;
        let view = document_conversation_view(&arrival, cabinet_section);
        let current_prompt = view.prompt;
        if conversation_id != utterance.conversation_id {
            return Ok(safe_conversation_response(
                arrival,
                current_prompt,
                "That answer belongs to a different Conversation, so I did not apply it. Please answer the current question.",
            ));
        }
        let Some(prompt) = current_prompt
            .clone()
            .filter(|prompt| prompt.id == utterance.linked_prompt)
        else {
            return Ok(safe_conversation_response(
                arrival,
                current_prompt,
                "That answer was linked to an earlier question, so I did not apply it. Please answer the current question.",
            ));
        };
        if !self
            .load_conversation_payload(household_id, conversation_id)?
            .deleted
        {
            self.add_conversation_message(
                household_id,
                conversation_id,
                "luna",
                &prompt.message,
                Some(arrival_id),
            )?;
            self.add_conversation_message(
                household_id,
                conversation_id,
                "member",
                &utterance.message,
                Some(arrival_id),
            )?;
        }
        let interpretation = interpreter.interpret(&prompt, &utterance);
        if interpretation.confidence != InterpretationConfidence::Confident {
            return Ok(ConversationTurnOutcome {
                status: ConversationTurnStatus::ClarificationRequired,
                accepted_direction: None,
                message: interpretation.ambiguity.unwrap_or_else(|| {
                    "I could not confidently apply that answer. Could you say it another way?"
                        .to_owned()
                }),
                next_prompt: Some(prompt),
                arrival,
                cloud_result: None,
            });
        }
        let mut valid_commands = interpretation
            .proposed_commands
            .into_iter()
            .filter(|command| validate_direction_for_prompt(&prompt, command).is_ok())
            .collect::<Vec<_>>();
        if valid_commands.len() > 1 {
            return Ok(ConversationTurnOutcome {
                status: ConversationTurnStatus::ClarificationRequired,
                accepted_direction: None,
                message:
                    "I found more than one possible direction. Please clarify which one you mean."
                        .to_owned(),
                next_prompt: Some(prompt),
                arrival,
                cloud_result: None,
            });
        }
        let Some(command) = valid_commands.pop() else {
            return Ok(safe_conversation_response(
                arrival,
                Some(prompt),
                "I could not safely apply that answer to the current question. Please answer it another way.",
            ));
        };

        if matches!(command, MemberDirectionCommand::UseCloudAssistance { .. }) {
            return Ok(ConversationTurnOutcome {
                status: ConversationTurnStatus::ActionPrepared,
                accepted_direction: Some(command),
                message:
                    "The Cloud Assistance direction is ready for the validated application service."
                        .to_owned(),
                next_prompt: Some(prompt),
                arrival,
                cloud_result: None,
            });
        }

        let updated = match &command {
            MemberDirectionCommand::ConfirmContextField { .. }
            | MemberDirectionCommand::RejectContextField { .. }
            | MemberDirectionCommand::SetContextField { .. } => {
                let direction = apply_context_direction(&arrival, &command)?;
                self.record_member_direction(household_id, arrival_id, direction, cabinet_section)?
            }
            MemberDirectionCommand::ConfirmFilingDecision => {
                let prepared = if arrival.review_card.filing_decision.is_none() {
                    let direction = provisional_context_direction(&arrival)
                        .ok_or(ConversationError::UnresolvedContext)?;
                    self.record_member_direction(
                        household_id,
                        arrival_id,
                        direction,
                        cabinet_section,
                    )?
                } else {
                    arrival.clone()
                };
                let decision = prepared
                    .review_card
                    .filing_decision
                    .as_ref()
                    .ok_or(ConversationError::UnresolvedContext)?;
                self.confirm_filing_decision(
                    household_id,
                    arrival_id,
                    FilingDecisionDirection {
                        file_name: decision.file_name.clone(),
                        cabinet_destination: decision.cabinet_destination.clone(),
                    },
                )?;
                self.file_document(household_id, arrival_id, cabinet_root)?
            }
            MemberDirectionCommand::UseCloudAssistance { .. } => {
                unreachable!("Cloud Assistance commands are prepared for the application service")
            }
            MemberDirectionCommand::ResolveDuplicate {
                decision,
                related_arrival_id,
            } => self.resolve_duplicate(
                household_id,
                arrival_id,
                *related_arrival_id,
                *decision,
                false,
            )?,
            MemberDirectionCommand::LearnFilingRule => {
                self.learn_filing_rule(household_id, arrival_id)?
            }
            MemberDirectionCommand::Decline => match prompt.purpose {
                ConversationPromptPurpose::ConfirmFilingDecision => {
                    self.decline_filing_decision(household_id, arrival_id)?
                }
                ConversationPromptPurpose::LearnFilingRule => {
                    self.decline_filing_rule(household_id, arrival_id)?
                }
                ConversationPromptPurpose::ClarifyContext
                | ConversationPromptPurpose::ChooseCloudAssistance
                | ConversationPromptPurpose::ResolveDuplicate => {
                    return Err(ConversationError::InvalidMemberDirection);
                }
            },
        };
        let next = document_conversation_view(&updated, cabinet_section);
        let status = match command {
            MemberDirectionCommand::ConfirmFilingDecision
                if updated.processing_state == DocumentProcessingState::Filed =>
            {
                ConversationTurnStatus::ActionCompleted
            }
            MemberDirectionCommand::LearnFilingRule => ConversationTurnStatus::ActionCompleted,
            MemberDirectionCommand::ResolveDuplicate { .. } => {
                ConversationTurnStatus::ActionCompleted
            }
            MemberDirectionCommand::Decline => ConversationTurnStatus::ActionRefused,
            _ => ConversationTurnStatus::AcceptedDirection,
        };
        let message = match status {
            ConversationTurnStatus::ActionRefused => {
                "Okay. I have not carried out that action. You can review the details or correct me."
                    .to_owned()
            }
            ConversationTurnStatus::ActionCompleted => next
                .completion_message
                .clone()
                .or_else(|| next.prompt.as_ref().map(|prompt| prompt.message.clone()))
                .unwrap_or_else(|| "Done.".to_owned()),
            ConversationTurnStatus::ActionPrepared => {
                "The action is ready for the validated application service.".to_owned()
            }
            _ => next
                .prompt
                .as_ref()
                .map(|prompt| prompt.message.clone())
                .unwrap_or_else(|| "Thanks. I have recorded that direction.".to_owned()),
        };
        Ok(ConversationTurnOutcome {
            status,
            accepted_direction: Some(command),
            message,
            next_prompt: next.prompt,
            arrival: updated,
            cloud_result: None,
        })
    }

    pub fn record_member_direction(
        &self,
        household_id: &str,
        arrival_id: i64,
        direction: DocumentContextDirection,
        cabinet_section: &str,
    ) -> Result<DocumentArrival, ConversationError> {
        let (conversation_id, mut payload) =
            self.load_document_arrival_payload(household_id, arrival_id)?;
        if !matches!(
            payload.processing_state,
            DocumentProcessingState::NeedsCloudConsent
                | DocumentProcessingState::WaitingForCloudAssistance
                | DocumentProcessingState::NeedsMemberDirection
        ) {
            return Err(ConversationError::NotFound);
        }
        payload.processing_state = DocumentProcessingState::NeedsMemberDirection;
        payload.context_direction = direction.normalized();
        payload.filing_decision_declined = false;
        payload.filing_decision = clarification_questions(&payload.context_direction)
            .is_empty()
            .then(|| propose_filing_decision(&payload, cabinet_section));
        self.save_document_arrival_payload(household_id, arrival_id, conversation_id, payload)
    }

    pub fn confirm_filing_decision(
        &self,
        household_id: &str,
        arrival_id: i64,
        direction: FilingDecisionDirection,
    ) -> Result<DocumentArrival, ConversationError> {
        let (conversation_id, mut payload) =
            self.load_document_arrival_payload(household_id, arrival_id)?;
        if payload.processing_state != DocumentProcessingState::NeedsMemberDirection {
            return Err(ConversationError::NotFound);
        }
        if !clarification_questions(&payload.context_direction).is_empty()
            || payload.filing_decision.is_none()
        {
            return Err(ConversationError::UnresolvedContext);
        }
        let file_name = direction.file_name.trim();
        let cabinet_destination = direction.cabinet_destination.trim();
        if !valid_file_name(file_name, &payload.original_name)
            || !valid_cabinet_destination(cabinet_destination, file_name)
        {
            return Err(ConversationError::InvalidCabinetDestination);
        }
        payload.filing_decision = Some(FilingDecisionReview {
            file_name: file_name.to_owned(),
            cabinet_destination: cabinet_destination.to_owned(),
            confirmed: true,
        });
        payload.filing_decision_declined = false;
        payload.processing_state = DocumentProcessingState::ReadyToFile;
        push_history(
            &mut payload.execution_history,
            "Member Direction confirmed the Filing Decision.",
        );
        self.save_document_arrival_payload(household_id, arrival_id, conversation_id, payload)
    }

    pub fn file_document(
        &self,
        household_id: &str,
        arrival_id: i64,
        cabinet_root: impl AsRef<Path>,
    ) -> Result<DocumentArrival, ConversationError> {
        let (conversation_id, mut payload) =
            self.load_document_arrival_payload(household_id, arrival_id)?;
        if payload.processing_state == DocumentProcessingState::Filed {
            let filed_original = payload
                .filed_original
                .as_ref()
                .ok_or(ConversationError::OriginalVerificationFailed)?;
            verify_existing_destination(&filed_original.final_path, &filed_original.checksum)?;
            self.release_staged_original_if_unreferenced(
                household_id,
                arrival_id,
                &payload.original_path,
            )?;
            return self.document_arrival(household_id, arrival_id, conversation_id, payload);
        }
        let resuming = matches!(
            payload.processing_state,
            DocumentProcessingState::Filing | DocumentProcessingState::CabinetUnavailable
        );
        if !matches!(
            payload.processing_state,
            DocumentProcessingState::ReadyToFile
                | DocumentProcessingState::Filing
                | DocumentProcessingState::CabinetUnavailable
        ) {
            return Err(ConversationError::NotFound);
        }
        let decision = payload
            .filing_decision
            .clone()
            .filter(|decision| decision.confirmed)
            .ok_or(ConversationError::UnresolvedContext)?;
        let cabinet_root = cabinet_root.as_ref();
        if !cabinet_root.is_dir() {
            payload.processing_state = DocumentProcessingState::CabinetUnavailable;
            push_history(
                &mut payload.execution_history,
                "Cabinet is unavailable; the verified Original remains staged for retry.",
            );
            return self.save_document_arrival_payload(
                household_id,
                arrival_id,
                conversation_id,
                payload,
            );
        }
        let staged = match fs::read(&payload.original_path) {
            Ok(staged) => staged,
            Err(_) if !cabinet_root.is_dir() => {
                payload.processing_state = DocumentProcessingState::CabinetUnavailable;
                push_history(
                    &mut payload.execution_history,
                    "Cabinet is unavailable; the verified Original remains staged for retry.",
                );
                return self.save_document_arrival_payload(
                    household_id,
                    arrival_id,
                    conversation_id,
                    payload,
                );
            }
            Err(_) => return Err(ConversationError::OriginalVerificationFailed),
        };
        if sha256(&staged) != payload.checksum {
            return Err(ConversationError::OriginalVerificationFailed);
        }
        let filing_result = (|| -> Result<PathBuf, ConversationError> {
            if !cabinet_root.is_dir() {
                return Err(ConversationError::DocumentUnavailable(io::Error::new(
                    io::ErrorKind::NotFound,
                    "Cabinet is unavailable",
                )));
            }
            let destination =
                safe_cabinet_destination(cabinet_root, &decision.cabinet_destination)?;

            if destination.exists() && !resuming {
                return Err(ConversationError::CabinetDestinationConflict);
            }
            if destination.exists() {
                verify_existing_destination(&destination, &payload.checksum)?;
            } else {
                if !resuming {
                    payload.processing_state = DocumentProcessingState::Filing;
                    push_history(
                        &mut payload.execution_history,
                        "Filing started after the destination and verified Original were checked.",
                    );
                    self.save_document_arrival_payload(
                        household_id,
                        arrival_id,
                        conversation_id,
                        payload.clone(),
                    )?;
                }
                let temporary = destination.with_file_name(format!(
                    ".luna-filing-{arrival_id}-{}.tmp",
                    payload.checksum
                ));
                if temporary.exists() && sha256(&fs::read(&temporary)?) != payload.checksum {
                    fs::remove_file(&temporary)?;
                }
                if !temporary.exists() {
                    let mut file = OpenOptions::new()
                        .write(true)
                        .create_new(true)
                        .open(&temporary)?;
                    file.write_all(&staged)?;
                    file.sync_all()?;
                }
                if sha256(&fs::read(&temporary)?) != payload.checksum {
                    return Err(ConversationError::OriginalVerificationFailed);
                }
                match fs::hard_link(&temporary, &destination) {
                    Ok(()) => {}
                    Err(error) if error.kind() == io::ErrorKind::AlreadyExists => {
                        return Err(ConversationError::CabinetDestinationConflict);
                    }
                    Err(error) => return Err(ConversationError::DocumentUnavailable(error)),
                }
                fs::remove_file(&temporary)?;
            }
            verify_existing_destination(&destination, &payload.checksum)?;
            Ok(destination)
        })();
        let destination = match filing_result {
            Ok(destination) => destination,
            Err(ConversationError::DocumentUnavailable(_)) => {
                payload.processing_state = DocumentProcessingState::CabinetUnavailable;
                push_history(
                    &mut payload.execution_history,
                    "Cabinet is unavailable; the verified Original remains staged for retry.",
                );
                return self.save_document_arrival_payload(
                    household_id,
                    arrival_id,
                    conversation_id,
                    payload,
                );
            }
            Err(error) => return Err(error),
        };

        let filed_original = FiledOriginal {
            arrival_id,
            conversation_id,
            original_name: payload.original_name.clone(),
            final_path: destination,
            checksum: payload.checksum.clone(),
            source_path: payload.source_path.clone(),
            filing_decision: decision.clone(),
        };
        payload.processing_state = DocumentProcessingState::Filed;
        payload.filed_original = Some(filed_original.clone());
        push_history(
            &mut payload.execution_history,
            &format!(
                "Verified Original filed at {}.",
                decision.cabinet_destination
            ),
        );
        let mut connection = self.connect()?;
        let transaction = connection.transaction()?;
        let protected_arrival = self.protect(household_id, &payload)?;
        let automatic = payload.automatic_rule_id.is_some();
        let protected_event = self.protect(
            household_id,
            &AuditEventPayload {
                kind: if automatic {
                    AuditEventKind::ExactMatchHandledAutomatically
                } else {
                    AuditEventKind::DocumentFiled
                },
                authority: if automatic {
                    AuditAuthority::FilingRule
                } else {
                    AuditAuthority::MemberDirection
                },
                subject: payload.original_name.clone(),
                outcome: if automatic {
                    format!(
                        "Automatically filed and verified at {}",
                        decision.cabinet_destination
                    )
                } else {
                    format!("Filed and verified at {}", decision.cabinet_destination)
                },
                filed_original,
            },
        )?;
        let updated = transaction.execute(
            "UPDATE document_arrivals SET protected_payload = ?1
              WHERE id = ?2 AND household_id = ?3",
            params![protected_arrival, arrival_id, household_id],
        )?;
        if updated == 0 {
            return Err(ConversationError::NotFound);
        }
        transaction.execute(
            "INSERT INTO audit_events (household_id, arrival_id, protected_payload)
             VALUES (?1, ?2, ?3)",
            params![household_id, arrival_id, protected_event],
        )?;
        transaction.commit()?;

        self.release_staged_original_if_unreferenced(
            household_id,
            arrival_id,
            &payload.original_path,
        )?;
        self.document_arrival(household_id, arrival_id, conversation_id, payload)
    }

    pub fn learn_filing_rule(
        &self,
        household_id: &str,
        arrival_id: i64,
    ) -> Result<DocumentArrival, ConversationError> {
        let (conversation_id, mut payload) =
            self.load_document_arrival_payload(household_id, arrival_id)?;
        if payload.processing_state != DocumentProcessingState::Filed
            || payload.automatic_rule_id.is_some()
        {
            return Err(ConversationError::InvalidMemberDirection);
        }
        if payload.learned_rule.is_some() {
            return self.document_arrival(household_id, arrival_id, conversation_id, payload);
        }
        let decision = payload
            .filing_decision
            .as_ref()
            .filter(|decision| decision.confirmed)
            .ok_or(ConversationError::UnresolvedContext)?;
        let mut rule = filing_rule_from_payload(&payload, decision)
            .ok_or(ConversationError::UnresolvedContext)?;
        let mut connection = self.connect()?;
        let transaction = connection.transaction()?;
        let protected_placeholder = self.protect(household_id, &rule)?;
        transaction.execute(
            "INSERT INTO filing_rules (household_id, protected_payload) VALUES (?1, ?2)",
            params![household_id, protected_placeholder],
        )?;
        rule.id = transaction.last_insert_rowid();
        let protected_rule = self.protect(household_id, &rule)?;
        transaction.execute(
            "UPDATE filing_rules SET protected_payload = ?1 WHERE id = ?2 AND household_id = ?3",
            params![protected_rule, rule.id, household_id],
        )?;
        payload.learned_rule = Some(rule);
        payload.filing_rule_declined = false;
        let protected_arrival = self.protect(household_id, &payload)?;
        transaction.execute(
            "UPDATE document_arrivals SET protected_payload = ?1
              WHERE id = ?2 AND household_id = ?3",
            params![protected_arrival, arrival_id, household_id],
        )?;
        transaction.commit()?;
        self.document_arrival(household_id, arrival_id, conversation_id, payload)
    }

    pub fn decline_filing_decision(
        &self,
        household_id: &str,
        arrival_id: i64,
    ) -> Result<DocumentArrival, ConversationError> {
        let (conversation_id, mut payload) =
            self.load_document_arrival_payload(household_id, arrival_id)?;
        if payload.processing_state != DocumentProcessingState::NeedsMemberDirection {
            return Err(ConversationError::InvalidMemberDirection);
        }
        if payload.filing_decision.is_none() {
            let arrival =
                self.document_arrival(household_id, arrival_id, conversation_id, payload.clone())?;
            if provisional_context_direction(&arrival).is_none() {
                return Err(ConversationError::InvalidMemberDirection);
            }
        }
        payload.filing_decision_declined = true;
        self.save_document_arrival_payload(household_id, arrival_id, conversation_id, payload)
    }

    pub fn decline_filing_rule(
        &self,
        household_id: &str,
        arrival_id: i64,
    ) -> Result<DocumentArrival, ConversationError> {
        let (conversation_id, mut payload) =
            self.load_document_arrival_payload(household_id, arrival_id)?;
        if payload.processing_state != DocumentProcessingState::Filed
            || payload.learned_rule.is_some()
        {
            return Err(ConversationError::InvalidMemberDirection);
        }
        payload.filing_rule_declined = true;
        self.save_document_arrival_payload(household_id, arrival_id, conversation_id, payload)
    }

    pub fn record_cloud_assistance_event(
        &self,
        household_id: &str,
        arrival_id: i64,
        summary: &str,
    ) -> Result<DocumentArrival, ConversationError> {
        let (conversation_id, mut payload) =
            self.load_document_arrival_payload(household_id, arrival_id)?;
        push_history(&mut payload.cloud_assistance_history, summary);
        self.save_document_arrival_payload(household_id, arrival_id, conversation_id, payload)
    }

    fn release_staged_original_if_unreferenced(
        &self,
        household_id: &str,
        arrival_id: i64,
        original_path: &Path,
    ) -> Result<(), ConversationError> {
        let connection = self.connect()?;
        let mut statement = connection.prepare(
            "SELECT id, protected_payload FROM document_arrivals
             WHERE household_id = ?1 AND id != ?2",
        )?;
        let protected_rows = statement
            .query_map(params![household_id, arrival_id], |row| {
                Ok((row.get::<_, i64>(0)?, row.get::<_, String>(1)?))
            })?
            .collect::<Result<Vec<_>, _>>()?;
        for (_, protected) in protected_rows {
            let other: DocumentArrivalPayload = self.open_protected(household_id, &protected)?;
            if other.processing_state != DocumentProcessingState::Filed
                && other.original_path == original_path
            {
                return Ok(());
            }
        }
        if original_path.is_file() {
            fs::remove_file(original_path)?;
            remove_empty_staging_directory(original_path);
        }
        Ok(())
    }

    pub fn resume_document_filings(
        &self,
        household_id: &str,
        cabinet_root: impl AsRef<Path>,
    ) -> Result<(), ConversationError> {
        let resumable = self
            .list_document_arrivals(household_id)?
            .into_iter()
            .filter(|arrival| {
                matches!(
                    arrival.processing_state,
                    DocumentProcessingState::ReadyToFile
                        | DocumentProcessingState::Filing
                        | DocumentProcessingState::CabinetUnavailable
                        | DocumentProcessingState::Filed
                )
            })
            .map(|arrival| arrival.id)
            .collect::<Vec<_>>();
        let mut first_error = None;
        for arrival_id in resumable {
            if let Err(error) = self.file_document(household_id, arrival_id, cabinet_root.as_ref())
            {
                if first_error.is_none() {
                    first_error = Some(error);
                }
            }
        }
        if let Some(error) = first_error {
            return Err(error);
        }
        Ok(())
    }

    fn matching_filing_rule(
        &self,
        household_id: &str,
        context: &DocumentContextDirection,
        extracted_text: Option<&str>,
    ) -> Result<Option<FilingRule>, ConversationError> {
        let connection = self.connect()?;
        let mut statement = connection.prepare(
            "SELECT protected_payload FROM filing_rules
              WHERE household_id = ?1
                AND NOT EXISTS (
                    SELECT 1 FROM portable_hidden_filing_rules AS hidden
                     WHERE hidden.household_id = filing_rules.household_id
                       AND hidden.rule_id = filing_rules.id
                )
              ORDER BY id DESC",
        )?;
        let rules = statement
            .query_map(params![household_id], |row| row.get::<_, String>(0))?
            .collect::<Result<Vec<_>, _>>()?;
        rules
            .into_iter()
            .map(|protected| self.open_protected::<FilingRule>(household_id, &protected))
            .find_map(|result| match result {
                Ok(rule)
                    if !rule.paused
                        && !rule.deleted
                        && filing_rule_matches(&rule, context, extracted_text) =>
                {
                    Some(Ok(rule))
                }
                Ok(_) => None,
                Err(error) => Some(Err(error)),
            })
            .transpose()
    }

    fn find_duplicate_review(
        &self,
        household_id: &str,
        arrival_id: i64,
        payload: &DocumentArrivalPayload,
    ) -> Result<Option<DuplicateReview>, ConversationError> {
        let connection = self.connect()?;
        let mut statement = connection.prepare(
            "SELECT id, protected_payload FROM document_arrivals
              WHERE household_id = ?1 AND id != ?2 ORDER BY id DESC",
        )?;
        let protected_rows = statement
            .query_map(params![household_id, arrival_id], |row| {
                Ok((row.get::<_, i64>(0)?, row.get::<_, String>(1)?))
            })?
            .collect::<Result<Vec<_>, _>>()?;
        let mut candidates = Vec::new();
        for (existing_id, protected) in protected_rows {
            let existing: DocumentArrivalPayload = self.open_protected(household_id, &protected)?;
            let kind = if existing.checksum == payload.checksum {
                Some(DuplicateKind::Exact)
            } else if possible_duplicate(&existing, payload) {
                Some(DuplicateKind::Possible)
            } else {
                None
            };
            if let Some(kind) = kind {
                candidates.push(DuplicateCandidate {
                    arrival_id: existing_id,
                    kind,
                    original_name: existing.original_name,
                    checksum: existing.checksum,
                    filed_destination: existing
                        .filed_original
                        .map(|filed| filed.filing_decision.cabinet_destination),
                });
            }
        }
        candidates.sort_by_key(|candidate| match candidate.kind {
            DuplicateKind::Exact => 0,
            DuplicateKind::Possible => 1,
        });
        Ok((!candidates.is_empty()).then_some(DuplicateReview { candidates }))
    }

    fn matching_duplicate_preference(
        &self,
        household_id: &str,
        payload: &DocumentArrivalPayload,
    ) -> Result<Option<DuplicatePreferencePayload>, ConversationError> {
        let connection = self.connect()?;
        let mut statement = connection.prepare(
            "SELECT protected_payload FROM duplicate_preferences
              WHERE household_id = ?1 ORDER BY id DESC",
        )?;
        let protected_rows = statement
            .query_map(params![household_id], |row| row.get::<_, String>(0))?
            .collect::<Result<Vec<_>, _>>()?;
        protected_rows
            .into_iter()
            .map(|protected| {
                self.open_protected::<DuplicatePreferencePayload>(household_id, &protected)
            })
            .find_map(|result| match result {
                Ok(preference)
                    if preference.checksum == payload.checksum
                        && duplicate_scope_matches(
                            &preference.context_direction,
                            &payload.context_direction,
                        ) =>
                {
                    Some(Ok(preference))
                }
                Ok(_) => None,
                Err(error) => Some(Err(error)),
            })
            .transpose()
    }

    fn resolve_duplicate_internal(
        &self,
        household_id: &str,
        arrival_id: i64,
        related_arrival_id: i64,
        decision: DuplicateDecision,
        remember_preference: bool,
        audit_kind: DuplicateAuditKind,
    ) -> Result<DocumentArrival, ConversationError> {
        let (conversation_id, mut payload) =
            self.load_document_arrival_payload(household_id, arrival_id)?;
        if payload.processing_state != DocumentProcessingState::PossibleDuplicate {
            return Err(ConversationError::DuplicateDecisionUnavailable);
        }
        let candidate = payload
            .duplicate_review
            .as_ref()
            .and_then(|review| {
                review
                    .candidates
                    .iter()
                    .find(|candidate| candidate.arrival_id == related_arrival_id)
            })
            .cloned()
            .ok_or(ConversationError::DuplicateDecisionUnavailable)?;
        if remember_preference && candidate.kind != DuplicateKind::Exact {
            return Err(ConversationError::DuplicateDecisionUnavailable);
        }
        payload.duplicate_review = None;
        payload.duplicate_resolution = Some(DuplicateResolution {
            decision,
            related_arrival_id,
            related_original_name: candidate.original_name.clone(),
            duplicate_kind: Some(candidate.kind),
        });
        payload.processing_state = match decision {
            DuplicateDecision::DiscardNew | DuplicateDecision::LinkCopies => {
                DocumentProcessingState::Dismissed
            }
            DuplicateDecision::KeepBoth | DuplicateDecision::UpdatedVersion => {
                DocumentProcessingState::NeedsMemberDirection
            }
        };
        let saved = self.save_document_arrival_payload(
            household_id,
            arrival_id,
            conversation_id,
            payload.clone(),
        )?;
        if remember_preference {
            let protected = self.protect(
                household_id,
                &DuplicatePreferencePayload {
                    checksum: candidate.checksum.clone(),
                    context_direction: payload.context_direction.clone(),
                    decision,
                    related_arrival_id,
                },
            )?;
            self.connect()?.execute(
                "INSERT INTO duplicate_preferences (household_id, protected_payload)
                 VALUES (?1, ?2)",
                params![household_id, protected],
            )?;
        }
        self.insert_duplicate_audit_event(
            household_id,
            arrival_id,
            &payload.original_name,
            &candidate,
            decision,
            audit_kind,
        )?;
        Ok(saved)
    }

    fn insert_duplicate_audit_event(
        &self,
        household_id: &str,
        arrival_id: i64,
        subject: &str,
        candidate: &DuplicateCandidate,
        decision: DuplicateDecision,
        kind: DuplicateAuditKind,
    ) -> Result<(), ConversationError> {
        let outcome = match decision {
            DuplicateDecision::KeepBoth => "kept both Originals",
            DuplicateDecision::LinkCopies => "linked both Originals without filing the new copy",
            DuplicateDecision::DiscardNew => "discarded the new duplicate Original",
            DuplicateDecision::UpdatedVersion => {
                "kept both Originals and marked the new copy as an updated version"
            }
        };
        let protected = self.protect(
            household_id,
            &DuplicateAuditPayload {
                kind,
                decision,
                subject: subject.to_owned(),
                outcome: format!(
                    "{}: {} ({})",
                    outcome,
                    candidate.original_name,
                    match candidate.kind {
                        DuplicateKind::Exact => "exact byte duplicate",
                        DuplicateKind::Possible => "possible duplicate with changed bytes",
                    }
                ),
                related_arrival_id: candidate.arrival_id,
            },
        )?;
        self.connect()?.execute(
            "INSERT INTO duplicate_events (household_id, arrival_id, protected_payload)
             VALUES (?1, ?2, ?3)",
            params![household_id, arrival_id, protected],
        )?;
        Ok(())
    }

    fn load_filing_rule(
        &self,
        household_id: &str,
        rule_id: i64,
    ) -> Result<(FilingRule, String), ConversationError> {
        let protected: Option<String> = self
            .connect()?
            .query_row(
                "SELECT protected_payload FROM filing_rules
                  WHERE id = ?1 AND household_id = ?2",
                params![rule_id, household_id],
                |row| row.get(0),
            )
            .optional()?;
        let protected = protected.ok_or(ConversationError::NotFound)?;
        let mut rule: FilingRule = self.open_protected(household_id, &protected)?;
        rule.id = rule_id;
        Ok((rule, protected))
    }

    fn filing_rule_summary(
        &self,
        household_id: &str,
        rule: FilingRule,
    ) -> Result<FilingRuleSummary, ConversationError> {
        let affected_documents = self
            .list_document_arrivals(household_id)?
            .into_iter()
            .filter(|arrival| {
                arrival
                    .review_card
                    .learned_rule
                    .as_ref()
                    .is_some_and(|learned| learned.id == rule.id)
            })
            .map(|arrival| arrival.original_name)
            .collect();
        Ok(FilingRuleSummary {
            id: rule.id,
            document_type: rule.document_type,
            service_provider: rule.service_provider,
            addressee: rule.addressee,
            property: rule.property,
            account: rule.account,
            file_name: rule.file_name,
            cabinet_destination: rule.cabinet_destination,
            teacher: if rule.teacher.is_empty() {
                "Member Direction".to_owned()
            } else {
                rule.teacher
            },
            created_at: rule.created_at,
            paused: rule.paused,
            deleted: rule.deleted,
            affected_documents,
        })
    }

    fn save_filing_rule(
        &self,
        household_id: &str,
        rule_id: i64,
        rule: &FilingRule,
        kind: FilingRuleAuditKind,
        outcome: &str,
    ) -> Result<(), ConversationError> {
        let protected_rule = self.protect(household_id, rule)?;
        let protected_event = self.protect(
            household_id,
            &FilingRuleAuditPayload {
                kind,
                subject: format!(
                    "{} from {} for {}",
                    rule.document_type, rule.service_provider, rule.addressee
                ),
                outcome: outcome.to_owned(),
            },
        )?;
        let mut connection = self.connect()?;
        let transaction = connection.transaction()?;
        let updated = transaction.execute(
            "UPDATE filing_rules SET protected_payload = ?1
              WHERE id = ?2 AND household_id = ?3",
            params![protected_rule, rule_id, household_id],
        )?;
        if updated == 0 {
            return Err(ConversationError::NotFound);
        }
        transaction.execute(
            "INSERT INTO filing_rule_events (household_id, rule_id, protected_payload)
             VALUES (?1, ?2, ?3)",
            params![household_id, rule_id, protected_event],
        )?;
        transaction.commit()?;
        Ok(())
    }

    fn insert_filing_rule_event(
        &self,
        household_id: &str,
        rule_id: i64,
        rule: &FilingRule,
        kind: FilingRuleAuditKind,
        outcome: &str,
    ) -> Result<(), ConversationError> {
        let protected_event = self.protect(
            household_id,
            &FilingRuleAuditPayload {
                kind,
                subject: format!(
                    "{} from {} for {}",
                    rule.document_type, rule.service_provider, rule.addressee
                ),
                outcome: outcome.to_owned(),
            },
        )?;
        self.connect()?.execute(
            "INSERT INTO filing_rule_events (household_id, rule_id, protected_payload)
             VALUES (?1, ?2, ?3)",
            params![household_id, rule_id, protected_event],
        )?;
        Ok(())
    }

    fn load_document_arrival_payload(
        &self,
        household_id: &str,
        arrival_id: i64,
    ) -> Result<(i64, DocumentArrivalPayload), ConversationError> {
        let stored: Option<(i64, String)> = self
            .connect()?
            .query_row(
                "SELECT conversation_id, protected_payload FROM document_arrivals
                  WHERE id = ?1 AND household_id = ?2",
                params![arrival_id, household_id],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .optional()?;
        let (conversation_id, protected) = stored.ok_or(ConversationError::NotFound)?;
        let mut payload: DocumentArrivalPayload = self.open_protected(household_id, &protected)?;
        payload.restore_legacy_original_path();
        Ok((conversation_id, payload))
    }

    fn save_document_arrival_payload(
        &self,
        household_id: &str,
        arrival_id: i64,
        conversation_id: i64,
        payload: DocumentArrivalPayload,
    ) -> Result<DocumentArrival, ConversationError> {
        let protected = self.protect(household_id, &payload)?;
        self.ensure_updated(
            "UPDATE document_arrivals SET protected_payload = ?1
              WHERE id = ?2 AND household_id = ?3",
            params![protected, arrival_id, household_id],
        )?;
        self.document_arrival(household_id, arrival_id, conversation_id, payload)
    }

    fn document_arrival(
        &self,
        household_id: &str,
        arrival_id: i64,
        conversation_id: i64,
        payload: DocumentArrivalPayload,
    ) -> Result<DocumentArrival, ConversationError> {
        let review_card = review_card(&payload);
        let authority_source = if payload.automatic_rule_id.is_some() {
            Some(AuditAuthority::FilingRule)
        } else if payload
            .filing_decision
            .as_ref()
            .is_some_and(|decision| decision.confirmed)
        {
            Some(AuditAuthority::MemberDirection)
        } else {
            None
        };
        Ok(DocumentArrival {
            id: arrival_id,
            household_id: household_id.to_owned(),
            conversation_id,
            original_name: payload.original_name,
            original_path: payload.original_path,
            source_path: payload.source_path,
            checksum: payload.checksum,
            media_type: payload.media_type,
            extracted_text: payload.extracted_text,
            review_card,
            processing_state: payload.processing_state,
            filed_original: payload.filed_original,
            duplicate_review: payload.duplicate_review,
            duplicate_resolution: payload.duplicate_resolution,
            authority_source,
            cloud_assistance_history: payload.cloud_assistance_history,
            execution_history: payload.execution_history,
            filing_decision_declined: payload.filing_decision_declined,
            filing_rule_declined: payload.filing_rule_declined,
        })
    }

    fn require_active_conversation(
        &self,
        household_id: &str,
        conversation_id: i64,
    ) -> Result<ConversationPayload, ConversationError> {
        let payload = self.load_conversation_payload(household_id, conversation_id)?;
        if payload.deleted {
            return Err(ConversationError::NotFound);
        }
        Ok(payload)
    }

    fn load_conversation_payload(
        &self,
        household_id: &str,
        conversation_id: i64,
    ) -> Result<ConversationPayload, ConversationError> {
        let protected: Option<String> = self
            .connect()?
            .query_row(
                "SELECT protected_payload FROM conversations
                  WHERE id = ?1 AND household_id = ?2",
                params![conversation_id, household_id],
                |row| row.get(0),
            )
            .optional()?;
        self.open_protected(household_id, &protected.ok_or(ConversationError::NotFound)?)
    }

    fn save_conversation_payload(
        &self,
        household_id: &str,
        conversation_id: i64,
        payload: &ConversationPayload,
    ) -> Result<(), ConversationError> {
        let protected = self.protect(household_id, payload)?;
        self.ensure_updated(
            "UPDATE conversations SET protected_payload = ?1
              WHERE id = ?2 AND household_id = ?3",
            params![protected, conversation_id, household_id],
        )
    }

    fn protect<T: Serialize>(
        &self,
        household_id: &str,
        value: &T,
    ) -> Result<String, ConversationError> {
        let plaintext = serde_json::to_vec(value)?;
        let protected = self
            .trusted_device
            .protect_household_state(household_id, &plaintext)?;
        Ok(serde_json::to_string(&protected)?)
    }

    fn open_protected<T: DeserializeOwned>(
        &self,
        household_id: &str,
        protected: &str,
    ) -> Result<T, ConversationError> {
        let protected: ProtectedHouseholdState = serde_json::from_str(protected)?;
        let plaintext = self
            .trusted_device
            .open_household_state(household_id, &protected)?;
        Ok(serde_json::from_slice(&plaintext)?)
    }

    fn ensure_updated<P>(&self, sql: &str, parameters: P) -> Result<(), ConversationError>
    where
        P: rusqlite::Params,
    {
        if self.connect()?.execute(sql, parameters)? == 0 {
            return Err(ConversationError::NotFound);
        }
        Ok(())
    }

    fn transition_cloud_assistance(
        &self,
        household_id: &str,
        arrival_id: i64,
        allowed_from: &[DocumentProcessingState],
        next: DocumentProcessingState,
    ) -> Result<DocumentArrival, ConversationError> {
        let (conversation_id, mut payload) =
            self.load_document_arrival_payload(household_id, arrival_id)?;
        if !allowed_from.contains(&payload.processing_state) {
            return Err(ConversationError::NotFound);
        }
        payload.processing_state = next;
        self.save_document_arrival_payload(household_id, arrival_id, conversation_id, payload)
    }

    fn connect(&self) -> rusqlite::Result<Connection> {
        Connection::open(&self.database)
    }

    fn preserve_original(
        &self,
        cabinet_root: &Path,
        checksum: &str,
        original_name: &str,
        original: &[u8],
    ) -> Result<PathBuf, ConversationError> {
        if !cabinet_root.is_dir() {
            return Err(ConversationError::DocumentUnavailable(io::Error::new(
                io::ErrorKind::NotFound,
                "Cabinet is unavailable",
            )));
        }
        let incoming = ensure_incoming_folder(cabinet_root).map_err(io::Error::other)?;
        let directory = incoming.join(checksum);
        fs::create_dir_all(&directory)?;
        let original_path = directory.join(original_name);
        match OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&original_path)
        {
            Ok(mut file) => {
                file.write_all(original)?;
                file.sync_all()?;
            }
            Err(error) if error.kind() == io::ErrorKind::AlreadyExists => {
                if fs::read(&original_path)? != original {
                    return Err(ConversationError::OriginalConflict);
                }
            }
            Err(error) => return Err(ConversationError::DocumentUnavailable(error)),
        }

        Ok(original_path)
    }
}

fn safe_conversation_response(
    arrival: DocumentArrival,
    current_prompt: Option<ConversationPrompt>,
    message: &str,
) -> ConversationTurnOutcome {
    ConversationTurnOutcome {
        status: ConversationTurnStatus::ClarificationRequired,
        accepted_direction: None,
        message: message.to_owned(),
        next_prompt: current_prompt,
        arrival,
        cloud_result: None,
    }
}

fn safe_cabinet_destination(
    cabinet_root: &Path,
    cabinet_destination: &str,
) -> Result<PathBuf, ConversationError> {
    let file_name = cabinet_destination
        .rsplit('/')
        .next()
        .ok_or(ConversationError::InvalidCabinetDestination)?;
    if !valid_cabinet_destination(cabinet_destination, file_name) || !cabinet_root.is_dir() {
        return Err(ConversationError::InvalidCabinetDestination);
    }
    let canonical_root = cabinet_root
        .canonicalize()
        .map_err(ConversationError::DocumentUnavailable)?;
    let components = cabinet_destination.split('/').collect::<Vec<_>>();
    let mut parent = canonical_root.clone();
    for component in &components[..components.len() - 1] {
        let candidate = parent.join(component);
        if !candidate.exists() {
            fs::create_dir(&candidate)?;
        }
        let canonical_candidate = candidate.canonicalize()?;
        if !canonical_candidate.is_dir() || !canonical_candidate.starts_with(&canonical_root) {
            return Err(ConversationError::InvalidCabinetDestination);
        }
        parent = canonical_candidate;
    }
    let destination = parent.join(file_name);
    if destination
        .parent()
        .is_none_or(|candidate| !candidate.starts_with(&canonical_root))
    {
        return Err(ConversationError::InvalidCabinetDestination);
    }
    Ok(destination)
}

fn verify_existing_destination(
    destination: &Path,
    expected_checksum: &str,
) -> Result<(), ConversationError> {
    if !destination.is_file() || sha256(&fs::read(destination)?) != expected_checksum {
        return Err(ConversationError::CabinetDestinationConflict);
    }
    Ok(())
}

fn remove_empty_staging_directory(original_path: &Path) {
    if let Some(directory) = original_path.parent() {
        let _ = fs::remove_dir(directory);
    }
}

fn find_checksum_file(root: &Path, checksum: &str) -> Option<PathBuf> {
    let entries = fs::read_dir(root).ok()?;
    for entry in entries.flatten() {
        let path = entry.path();
        if path.file_name().is_some_and(|name| name == ".luna") {
            continue;
        }
        if path.is_dir() {
            if let Some(found) = find_checksum_file(&path, checksum) {
                return Some(found);
            }
        } else if path.is_file()
            && fs::read(&path)
                .ok()
                .is_some_and(|contents| sha256(&contents) == checksum)
        {
            return Some(path);
        }
    }
    None
}

fn detected_media_type(original: &[u8]) -> Result<&'static str, ConversationError> {
    if original.starts_with(b"%PDF-") {
        Ok("application/pdf")
    } else if original.starts_with(&[0xFF, 0xD8, 0xFF]) {
        image::load_from_memory_with_format(original, ImageFormat::Jpeg)
            .map_err(|_| ConversationError::InvalidDocument)?;
        Ok("image/jpeg")
    } else if original.starts_with(b"\x89PNG\r\n\x1a\n") {
        image::load_from_memory_with_format(original, ImageFormat::Png)
            .map_err(|_| ConversationError::InvalidDocument)?;
        Ok("image/png")
    } else {
        Err(ConversationError::InvalidDocument)
    }
}

fn sha256(original: &[u8]) -> String {
    format!("{:x}", Sha256::digest(original))
}

fn extract_digital_pdf_text(
    media_type: &str,
    original: &[u8],
) -> Result<Option<String>, ConversationError> {
    if media_type != "application/pdf" {
        return Ok(None);
    }
    match catch_unwind(AssertUnwindSafe(|| {
        pdf_extract::extract_text_from_mem(original)
    })) {
        Ok(Ok(text)) => {
            let text = text.trim().to_owned();
            Ok((!text.is_empty()).then_some(text))
        }
        Ok(Err(_)) => lopdf_text(original)
            .map(Some)
            .ok_or(ConversationError::InvalidDocument),
        Err(_) => Ok(lopdf_text(original)),
    }
}

fn lopdf_text(original: &[u8]) -> Option<String> {
    if !original.windows(5).any(|window| window == b"%%EOF") {
        return None;
    }
    let result = catch_unwind(AssertUnwindSafe(|| {
        let document = lopdf::Document::load_mem(original).ok()?;
        let pages = document.get_pages();
        if pages.is_empty() {
            return None;
        }
        let page_numbers = pages.keys().copied().collect::<Vec<_>>();
        document.extract_text(&page_numbers).ok()
    }))
    .ok()
    .flatten()?;
    let text = result.trim().to_owned();
    (!text.is_empty()).then_some(text)
}

fn extract_local_text(
    media_type: &str,
    original_path: &Path,
    extracted_pdf_text: Option<String>,
    local_ocr: &dyn LocalOcr,
) -> Option<String> {
    match media_type {
        "application/pdf" => {
            extracted_pdf_text.or_else(|| local_ocr.extract_text(original_path, media_type))
        }
        "image/jpeg" | "image/png" => local_ocr.extract_text(original_path, media_type),
        _ => None,
    }
}

fn local_context_direction(extracted_text: Option<&str>) -> DocumentContextDirection {
    let mut direction = DocumentContextDirection::default();
    let Some(extracted_text) = extracted_text else {
        return direction;
    };
    for segment in extracted_text.split(['\n', ';']) {
        let Some((label, value)) = segment.split_once(':') else {
            continue;
        };
        let value = value.trim();
        if value.is_empty() {
            continue;
        }
        match label.trim().to_ascii_lowercase().as_str() {
            "document type" => direction.document_type = Some(value.to_owned()),
            "service provider" | "provider" => {
                direction.service_provider = Some(value.to_owned());
            }
            "addressee" => direction.addressee = Some(value.to_owned()),
            "property" | "property address" | "address" => {
                direction.property = Some(value.to_owned());
            }
            "account" | "account number" => direction.account = Some(value.to_owned()),
            "amount" => direction.amount = Some(value.to_owned()),
            "relevant date" | "date" => direction.relevant_dates.push(value.to_owned()),
            _ => {}
        }
    }
    direction
}

fn review_card(payload: &DocumentArrivalPayload) -> ReviewCard {
    let mut evidence = vec![
        ReviewEvidence {
            label: "Original name".to_owned(),
            value: payload.original_name.clone(),
        },
        ReviewEvidence {
            label: "Detected type".to_owned(),
            value: media_type_label(&payload.media_type).to_owned(),
        },
        ReviewEvidence {
            label: "SHA-256".to_owned(),
            value: if payload.checksum.is_empty() {
                "Not recorded before Original staging".to_owned()
            } else {
                payload.checksum.clone()
            },
        },
    ];
    if let Some(text) = &payload.extracted_text {
        evidence.push(ReviewEvidence {
            label: "Extracted text".to_owned(),
            value: text.clone(),
        });
    } else {
        evidence.push(ReviewEvidence {
            label: "Local inspection".to_owned(),
            value: "No text could be read locally.".to_owned(),
        });
    }
    if payload.processing_state == DocumentProcessingState::CabinetUnavailable {
        evidence.push(ReviewEvidence {
            label: "Recovery status".to_owned(),
            value: "The untouched Original remains staged with this checksum. Luna will retry only the confirmed Cabinet Destination.".to_owned(),
        });
    }
    let context = &payload.context_direction;
    let questions = clarification_questions(context);
    ReviewCard {
        confidence_state: if questions.is_empty() {
            ConfidenceState::Confirmed
        } else if payload.extracted_text.is_some() {
            ConfidenceState::NeedsChecking
        } else {
            ConfidenceState::Unknown
        },
        evidence,
        uncertainties: if questions.is_empty() {
            Vec::new()
        } else {
            vec!["Luna needs your direction before filing this Original.".to_owned()]
        },
        proposed_cabinet_destination: payload
            .filing_decision
            .as_ref()
            .map(|decision| decision.cabinet_destination.clone()),
        context: DocumentContextReview {
            document_type: review_field(&context.document_type, context.document_type_resolved),
            service_provider: review_field(
                &context.service_provider,
                context.service_provider_resolved,
            ),
            service_provider_relevance: review_field(
                &context
                    .service_provider_relevance
                    .as_ref()
                    .map(|relevance| relevance.explanation.clone()),
                context.service_provider_relevance.is_some(),
            ),
            addressee: review_field(&context.addressee, context.addressee_resolved),
            property: review_field(&context.property, context.property_resolved),
            property_relevance: review_field(
                &context
                    .property_relevance
                    .as_ref()
                    .map(|relevance| relevance.explanation.clone()),
                context.property_relevance.is_some(),
            ),
            account: review_field(&context.account, context.account_resolved),
            amount: review_field(&context.amount, context.amount_resolved),
            relevant_dates: context
                .relevant_dates
                .iter()
                .map(|date| review_field(&Some(date.clone()), context.relevant_dates_resolved))
                .collect(),
        },
        questions,
        filing_decision: payload.filing_decision.clone(),
        learned_rule: payload.learned_rule.clone(),
    }
}

fn document_conversation_view(
    arrival: &DocumentArrival,
    cabinet_section: &str,
) -> DocumentConversationView {
    let understanding = document_understanding(arrival);
    if matches!(
        arrival.processing_state,
        DocumentProcessingState::NeedsCloudConsent
            | DocumentProcessingState::WaitingForCloudAssistance
    ) {
        let message = format!(
            "{understanding}\n\nLocal Evidence is not enough to interpret this Document safely. Review the disclosed Intelligence Provider and consent choices below, or say “Keep local”."
        );
        return DocumentConversationView {
            understanding,
            prompt: Some(ConversationPrompt {
                id: conversation_prompt_id(arrival.id, "choose-cloud-assistance", &message),
                purpose: ConversationPromptPurpose::ChooseCloudAssistance,
                subject: arrival.original_name.clone(),
                message,
                expected_response: ConversationExpectedResponse::Choice,
                allowed_actions: vec![
                    ConversationAction::KeepLocal,
                    ConversationAction::ReviewDetails,
                ],
                linked_document_arrival: arrival.id,
                evidence_summary: evidence_summary(arrival),
                context_field: None,
                related_arrival_id: None,
            }),
            completion_message: None,
        };
    }

    if arrival.processing_state == DocumentProcessingState::PossibleDuplicate {
        if let Some(candidate) = arrival
            .duplicate_review
            .as_ref()
            .and_then(|review| review.candidates.first())
        {
            let duplicate_kind = match candidate.kind {
                DuplicateKind::Exact => "an exact byte duplicate",
                DuplicateKind::Possible => "a possible duplicate with changed bytes",
            };
            let message = format!(
                "{understanding}\n\nThis may be {duplicate_kind} of {}. Say “Keep both”, “Link copies”, “Discard new”, or “Updated version”.",
                candidate.original_name
            );
            return DocumentConversationView {
                understanding,
                prompt: Some(ConversationPrompt {
                    id: conversation_prompt_id(arrival.id, "resolve-duplicate", &message),
                    purpose: ConversationPromptPurpose::ResolveDuplicate,
                    subject: arrival.original_name.clone(),
                    message,
                    expected_response: ConversationExpectedResponse::Choice,
                    allowed_actions: vec![
                        ConversationAction::KeepBoth,
                        ConversationAction::LinkCopies,
                        ConversationAction::DiscardNew,
                        ConversationAction::UpdatedVersion,
                        ConversationAction::ReviewDetails,
                    ],
                    linked_document_arrival: arrival.id,
                    evidence_summary: evidence_summary(arrival),
                    context_field: None,
                    related_arrival_id: Some(candidate.arrival_id),
                }),
                completion_message: None,
            };
        }
    }

    if arrival.processing_state == DocumentProcessingState::Filed {
        let destination = arrival
            .filed_original
            .as_ref()
            .map(|filed| filed.filing_decision.cabinet_destination.as_str())
            .unwrap_or("the Cabinet");
        let completion = format!("Done. I filed the verified Original in:\n{destination}");
        if arrival.review_card.learned_rule.is_none()
            && !arrival.filing_rule_declined
            && can_learn_filing_rule(arrival)
        {
            let message =
                format!("{completion}\n\nShould I handle future matching documents the same way?");
            return DocumentConversationView {
                understanding,
                prompt: Some(ConversationPrompt {
                    id: conversation_prompt_id(arrival.id, "learn-filing-rule", &message),
                    purpose: ConversationPromptPurpose::LearnFilingRule,
                    subject: arrival.original_name.clone(),
                    message,
                    expected_response: ConversationExpectedResponse::Confirmation,
                    allowed_actions: vec![
                        ConversationAction::AlwaysDoThis,
                        ConversationAction::No,
                        ConversationAction::ReviewDetails,
                    ],
                    linked_document_arrival: arrival.id,
                    evidence_summary: evidence_summary(arrival),
                    context_field: None,
                    related_arrival_id: None,
                }),
                completion_message: None,
            };
        }
        return DocumentConversationView {
            understanding,
            prompt: None,
            completion_message: Some(if arrival.filing_rule_declined {
                format!("{completion}\n\nI will not make this a Filing Rule.")
            } else {
                completion
            }),
        };
    }

    if arrival.processing_state == DocumentProcessingState::NeedsMemberDirection
        && !arrival.filing_decision_declined
    {
        if let Some(decision) = provisional_filing_decision(arrival, cabinet_section) {
            return filing_confirmation_view(arrival, understanding, &decision);
        }
        if let Some(question) = next_material_question(arrival) {
            let value = context_field_value(&arrival.review_card.context, question.field);
            let confirmation = value.is_some()
                && !matches!(
                    question.field,
                    ContextField::ServiceProviderRelevance | ContextField::PropertyRelevance
                );
            let question_text = if confirmation {
                confirmation_question(question.field, value.as_deref().unwrap_or_default())
            } else {
                question.prompt.clone()
            };
            let message = format!("{understanding}\n\n{question_text}");
            return DocumentConversationView {
                understanding: understanding.clone(),
                prompt: Some(ConversationPrompt {
                    id: conversation_prompt_id(
                        arrival.id,
                        &format!(
                            "context-{}-{}",
                            context_field_key(question.field),
                            if confirmation { "confirm" } else { "value" }
                        ),
                        &message,
                    ),
                    purpose: ConversationPromptPurpose::ClarifyContext,
                    subject: arrival.original_name.clone(),
                    message,
                    expected_response: if confirmation {
                        ConversationExpectedResponse::Confirmation
                    } else {
                        ConversationExpectedResponse::ContextValue
                    },
                    allowed_actions: if confirmation {
                        vec![
                            ConversationAction::Yes,
                            ConversationAction::No,
                            ConversationAction::ReviewDetails,
                        ]
                    } else if matches!(
                        question.field,
                        ContextField::Property | ContextField::Account
                    ) {
                        vec![ConversationAction::No, ConversationAction::ReviewDetails]
                    } else {
                        vec![ConversationAction::ReviewDetails]
                    },
                    linked_document_arrival: arrival.id,
                    evidence_summary: evidence_summary(arrival),
                    context_field: Some(question.field),
                    related_arrival_id: None,
                }),
                completion_message: None,
            };
        }
        if let Some(decision) = arrival
            .review_card
            .filing_decision
            .as_ref()
            .filter(|decision| !decision.confirmed && !arrival.filing_decision_declined)
        {
            return filing_confirmation_view(arrival, understanding, decision);
        }
    }

    DocumentConversationView {
        understanding,
        prompt: None,
        completion_message: if arrival.filing_decision_declined {
            Some(
                "I have not filed this Original. You can correct the proposal in Review details."
                    .to_owned(),
            )
        } else {
            match arrival.processing_state {
                DocumentProcessingState::CabinetUnavailable => Some(
                    "The Cabinet is unavailable. I kept the Original safely staged and will retry."
                        .to_owned(),
                ),
                _ => None,
            }
        },
    }
}

fn filing_confirmation_view(
    arrival: &DocumentArrival,
    understanding: String,
    decision: &FilingDecisionReview,
) -> DocumentConversationView {
    let message = format!(
        "{understanding}\n\nI can file it in:\n{}\n\nIs that correct?",
        decision.cabinet_destination
    );
    DocumentConversationView {
        understanding: understanding.clone(),
        prompt: Some(ConversationPrompt {
            id: conversation_prompt_id(
                arrival.id,
                "confirm-filing",
                &format!("{message}\n{}", decision.file_name),
            ),
            purpose: ConversationPromptPurpose::ConfirmFilingDecision,
            subject: arrival.original_name.clone(),
            message,
            expected_response: ConversationExpectedResponse::Confirmation,
            allowed_actions: vec![
                ConversationAction::Yes,
                ConversationAction::No,
                ConversationAction::ReviewDetails,
            ],
            linked_document_arrival: arrival.id,
            evidence_summary: evidence_summary(arrival),
            context_field: None,
            related_arrival_id: None,
        }),
        completion_message: None,
    }
}

fn provisional_filing_decision(
    arrival: &DocumentArrival,
    cabinet_section: &str,
) -> Option<FilingDecisionReview> {
    if arrival.review_card.filing_decision.is_some() {
        return None;
    }
    let context = provisional_context_direction(arrival)?;
    Some(propose_filing_decision_from_context(
        &arrival.original_name,
        &context,
        cabinet_section,
    ))
}

fn provisional_context_direction(arrival: &DocumentArrival) -> Option<DocumentContextDirection> {
    let mut direction = direction_from_review(arrival);
    if direction.document_type.is_none()
        || direction.service_provider.is_none()
        || direction.addressee.is_none()
        || (direction.property.is_none() && direction.account.is_none())
    {
        return None;
    }

    direction.document_type_resolved = true;
    direction.service_provider_resolved = true;
    direction.addressee_resolved = true;
    direction.property_resolved = true;
    direction.account_resolved = true;
    direction.amount_resolved = true;
    direction.relevant_dates_resolved = true;
    if direction.service_provider_relevance.is_none() {
        direction.service_provider_relevance =
            direction
                .service_provider
                .clone()
                .map(|subject| ContextRelevanceDirection {
                    subject,
                    explanation: "Confirmed by the member for this filing".to_owned(),
                });
    }
    if direction.property_relevance.is_none() {
        direction.property_relevance =
            direction
                .property
                .clone()
                .map(|subject| ContextRelevanceDirection {
                    subject,
                    explanation: "Confirmed by the member for this filing".to_owned(),
                });
    }
    Some(direction)
}

fn can_learn_filing_rule(arrival: &DocumentArrival) -> bool {
    let context = &arrival.review_card.context;
    context.document_type.value.is_some()
        && context.service_provider.value.is_some()
        && context.addressee.value.is_some()
        && (context.property.value.is_some() || context.account.value.is_some())
}

fn document_understanding(arrival: &DocumentArrival) -> String {
    let context = &arrival.review_card.context;
    let mut parts = Vec::new();
    if let Some(document_type) = &context.document_type.value {
        let document_type = document_type.to_lowercase();
        parts.push(format!(
            "{} {document_type}",
            indefinite_article(&document_type)
        ));
    } else {
        let media_type = media_type_label(&arrival.media_type).to_lowercase();
        parts.push(format!("{} {media_type}", indefinite_article(&media_type)));
    }
    if let Some(provider) = &context.service_provider.value {
        parts.push(format!("from {provider}"));
    }
    if let Some(property) = &context.property.value {
        parts.push(format!("for {property}"));
    }
    if let Some(account) = &context.account.value {
        parts.push(format!("on account {account}"));
    }
    let mut message = format!("This appears to be {}.", parts.join(" "));
    if let Some(addressee) = &context.addressee.value {
        message.push_str(&format!(" It is addressed to {addressee}."));
    }
    message
}

fn conversation_prompt_id(arrival_id: i64, kind: &str, material: &str) -> String {
    let digest = sha256(material.as_bytes());
    format!("document:{arrival_id}:{kind}:{}", &digest[..16])
}

fn push_history(history: &mut Vec<String>, entry: &str) {
    if history.last().is_none_or(|previous| previous != entry) {
        history.push(entry.to_owned());
    }
}

fn indefinite_article(value: &str) -> &'static str {
    if value
        .chars()
        .next()
        .is_some_and(|character| "aeiou".contains(character))
    {
        "an"
    } else {
        "a"
    }
}

fn evidence_summary(arrival: &DocumentArrival) -> Vec<String> {
    arrival
        .review_card
        .evidence
        .iter()
        .take(3)
        .map(|evidence| format!("{}: {}", evidence.label, evidence.value))
        .collect()
}

fn next_material_question(arrival: &DocumentArrival) -> Option<&ClarificationQuestion> {
    arrival.review_card.questions.iter().find(|question| {
        question.field != ContextField::Amount
            && (question.field != ContextField::RelevantDates
                || !arrival.review_card.context.relevant_dates.is_empty())
    })
}

fn context_field_value(context: &DocumentContextReview, field: ContextField) -> Option<String> {
    match field {
        ContextField::DocumentType => context.document_type.value.clone(),
        ContextField::ServiceProvider => context.service_provider.value.clone(),
        ContextField::ServiceProviderRelevance => context.service_provider_relevance.value.clone(),
        ContextField::Addressee => context.addressee.value.clone(),
        ContextField::Property => context.property.value.clone(),
        ContextField::PropertyRelevance => context.property_relevance.value.clone(),
        ContextField::Account => context.account.value.clone(),
        ContextField::Amount => context.amount.value.clone(),
        ContextField::RelevantDates => context
            .relevant_dates
            .iter()
            .filter_map(|date| date.value.clone())
            .next(),
    }
}

fn confirmation_question(field: ContextField, value: &str) -> String {
    match field {
        ContextField::DocumentType => format!("Is this a {value}?"),
        ContextField::ServiceProvider => format!("Is {value} the Service Provider?"),
        ContextField::Addressee => format!("Is it addressed to {value}?"),
        ContextField::Property => format!("Is this for {value}?"),
        ContextField::Account => format!("Is this for account {value}?"),
        ContextField::RelevantDates => format!("Should I use {value} as the relevant date?"),
        ContextField::Amount => format!("Is the amount {value}?"),
        ContextField::ServiceProviderRelevance | ContextField::PropertyRelevance => {
            "How is this relevant to the Household?".to_owned()
        }
    }
}

fn context_field_key(field: ContextField) -> &'static str {
    match field {
        ContextField::DocumentType => "document-type",
        ContextField::ServiceProvider => "service-provider",
        ContextField::ServiceProviderRelevance => "service-provider-relevance",
        ContextField::Addressee => "addressee",
        ContextField::Property => "property",
        ContextField::PropertyRelevance => "property-relevance",
        ContextField::Account => "account",
        ContextField::Amount => "amount",
        ContextField::RelevantDates => "relevant-dates",
    }
}

fn validate_direction_for_prompt(
    prompt: &ConversationPrompt,
    command: &MemberDirectionCommand,
) -> Result<(), ConversationError> {
    let valid = match command {
        MemberDirectionCommand::ConfirmContextField { field }
        | MemberDirectionCommand::RejectContextField { field }
        | MemberDirectionCommand::SetContextField { field, .. } => {
            prompt.purpose == ConversationPromptPurpose::ClarifyContext
                && prompt.context_field == Some(*field)
                && (!matches!(command, MemberDirectionCommand::ConfirmContextField { .. })
                    || prompt.expected_response == ConversationExpectedResponse::Confirmation)
        }
        MemberDirectionCommand::ConfirmFilingDecision => {
            prompt.purpose == ConversationPromptPurpose::ConfirmFilingDecision
        }
        MemberDirectionCommand::UseCloudAssistance { .. } => {
            prompt.purpose == ConversationPromptPurpose::ChooseCloudAssistance
        }
        MemberDirectionCommand::ResolveDuplicate {
            related_arrival_id, ..
        } => {
            prompt.purpose == ConversationPromptPurpose::ResolveDuplicate
                && prompt.related_arrival_id == Some(*related_arrival_id)
        }
        MemberDirectionCommand::LearnFilingRule => {
            prompt.purpose == ConversationPromptPurpose::LearnFilingRule
        }
        MemberDirectionCommand::Decline => matches!(
            prompt.purpose,
            ConversationPromptPurpose::ConfirmFilingDecision
                | ConversationPromptPurpose::LearnFilingRule
        ),
    };
    if valid {
        Ok(())
    } else {
        Err(ConversationError::InvalidMemberDirection)
    }
}

fn apply_context_direction(
    arrival: &DocumentArrival,
    command: &MemberDirectionCommand,
) -> Result<DocumentContextDirection, ConversationError> {
    let mut direction = direction_from_review(arrival);
    match command {
        MemberDirectionCommand::ConfirmContextField { field } => {
            if context_field_value(&arrival.review_card.context, *field).is_none() {
                return Err(ConversationError::InvalidMemberDirection);
            }
            set_context_resolved(&mut direction, *field, true);
        }
        MemberDirectionCommand::RejectContextField { field } => {
            set_context_value(&mut direction, *field, None)?;
            set_context_resolved(&mut direction, *field, false);
        }
        MemberDirectionCommand::SetContextField { field, value } => {
            let value = value
                .as_ref()
                .map(|value| value.trim().to_owned())
                .filter(|value| !value.is_empty());
            if value.is_none() && !matches!(field, ContextField::Property | ContextField::Account) {
                return Err(ConversationError::InvalidMemberDirection);
            }
            set_context_value(&mut direction, *field, value)?;
            set_context_resolved(&mut direction, *field, true);
        }
        _ => return Err(ConversationError::InvalidMemberDirection),
    }
    Ok(direction)
}

fn direction_from_review(arrival: &DocumentArrival) -> DocumentContextDirection {
    let context = &arrival.review_card.context;
    let confirmed = |field: &ReviewField| field.confidence_state == ConfidenceState::Confirmed;
    DocumentContextDirection {
        document_type: context.document_type.value.clone(),
        document_type_resolved: confirmed(&context.document_type),
        service_provider: context.service_provider.value.clone(),
        service_provider_resolved: confirmed(&context.service_provider),
        addressee: context.addressee.value.clone(),
        addressee_resolved: confirmed(&context.addressee),
        property: context.property.value.clone(),
        property_resolved: confirmed(&context.property),
        account: context.account.value.clone(),
        account_resolved: confirmed(&context.account),
        amount: context.amount.value.clone(),
        amount_resolved: confirmed(&context.amount) || context.amount.value.is_none(),
        relevant_dates: context
            .relevant_dates
            .iter()
            .filter_map(|date| date.value.clone())
            .collect(),
        relevant_dates_resolved: context.relevant_dates.is_empty()
            || context.relevant_dates.iter().all(confirmed),
        service_provider_relevance: context.service_provider_relevance.value.as_ref().map(
            |explanation| ContextRelevanceDirection {
                subject: context.service_provider.value.clone().unwrap_or_default(),
                explanation: explanation.clone(),
            },
        ),
        property_relevance: context
            .property_relevance
            .value
            .as_ref()
            .map(|explanation| ContextRelevanceDirection {
                subject: context.property.value.clone().unwrap_or_default(),
                explanation: explanation.clone(),
            }),
    }
}

fn set_context_resolved(
    direction: &mut DocumentContextDirection,
    field: ContextField,
    resolved: bool,
) {
    match field {
        ContextField::DocumentType => direction.document_type_resolved = resolved,
        ContextField::ServiceProvider => direction.service_provider_resolved = resolved,
        ContextField::Addressee => direction.addressee_resolved = resolved,
        ContextField::Property => direction.property_resolved = resolved,
        ContextField::Account => direction.account_resolved = resolved,
        ContextField::Amount => direction.amount_resolved = resolved,
        ContextField::RelevantDates => direction.relevant_dates_resolved = resolved,
        ContextField::ServiceProviderRelevance | ContextField::PropertyRelevance => {}
    }
}

fn set_context_value(
    direction: &mut DocumentContextDirection,
    field: ContextField,
    value: Option<String>,
) -> Result<(), ConversationError> {
    match field {
        ContextField::DocumentType => direction.document_type = value,
        ContextField::ServiceProvider => {
            if direction.service_provider != value {
                direction.service_provider_relevance = None;
            }
            direction.service_provider = value;
        }
        ContextField::ServiceProviderRelevance => {
            let subject = direction
                .service_provider
                .clone()
                .ok_or(ConversationError::InvalidMemberDirection)?;
            direction.service_provider_relevance =
                value.map(|explanation| ContextRelevanceDirection {
                    subject,
                    explanation,
                });
        }
        ContextField::Addressee => direction.addressee = value,
        ContextField::Property => {
            if direction.property != value {
                direction.property_relevance = None;
            }
            direction.property = value;
        }
        ContextField::PropertyRelevance => {
            let subject = direction
                .property
                .clone()
                .ok_or(ConversationError::InvalidMemberDirection)?;
            direction.property_relevance = value.map(|explanation| ContextRelevanceDirection {
                subject,
                explanation,
            });
        }
        ContextField::Account => direction.account = value,
        ContextField::Amount => direction.amount = value,
        ContextField::RelevantDates => {
            direction.relevant_dates = value.into_iter().collect();
        }
    }
    Ok(())
}

fn filing_rule_from_payload(
    payload: &DocumentArrivalPayload,
    decision: &FilingDecisionReview,
) -> Option<FilingRule> {
    let context = &payload.context_direction;
    let document_type = context.document_type.clone()?;
    let service_provider = context.service_provider.clone()?;
    let addressee = context.addressee.clone()?;
    if context.property.is_none() && context.account.is_none() {
        return None;
    }
    Some(FilingRule {
        id: 0,
        document_type,
        service_provider,
        addressee,
        property: context.property.clone(),
        account: context.account.clone(),
        file_name: decision.file_name.clone(),
        cabinet_destination: decision.cabinet_destination.clone(),
        teacher: "Member Direction".to_owned(),
        created_at: current_timestamp(),
        paused: false,
        deleted: false,
    })
}

fn current_timestamp() -> String {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs().to_string())
        .unwrap_or_else(|_| "0".to_owned())
}

impl FilingRuleUpdate {
    fn normalized(self) -> Self {
        Self {
            document_type: self.document_type.trim().to_owned(),
            service_provider: self.service_provider.trim().to_owned(),
            addressee: self.addressee.trim().to_owned(),
            property: self
                .property
                .map(|value| value.trim().to_owned())
                .filter(|value| !value.is_empty()),
            account: self
                .account
                .map(|value| value.trim().to_owned())
                .filter(|value| !value.is_empty()),
            file_name: self.file_name.trim().to_owned(),
            cabinet_destination: self.cabinet_destination.trim().to_owned(),
        }
    }
}

fn filing_rule_matches(
    rule: &FilingRule,
    context: &DocumentContextDirection,
    extracted_text: Option<&str>,
) -> bool {
    let structured_match = rule.document_type
        == context.document_type.as_deref().unwrap_or_default()
        && rule.service_provider == context.service_provider.as_deref().unwrap_or_default()
        && rule.addressee == context.addressee.as_deref().unwrap_or_default()
        && rule.property == context.property
        && rule.account == context.account;
    let context_is_unresolved = context.document_type.is_none()
        && context.service_provider.is_none()
        && context.addressee.is_none()
        && context.property.is_none()
        && context.account.is_none();
    structured_match
        || (context_is_unresolved
            && extracted_text.is_some_and(|text| filing_rule_matches_text(rule, text)))
}

fn possible_duplicate(
    existing: &DocumentArrivalPayload,
    incoming: &DocumentArrivalPayload,
) -> bool {
    if existing.media_type != incoming.media_type || existing.checksum == incoming.checksum {
        return false;
    }
    if !existing
        .original_name
        .eq_ignore_ascii_case(&incoming.original_name)
    {
        return false;
    }
    let existing_context = &existing.context_direction;
    let incoming_context = &incoming.context_direction;
    let core_context_matches = [
        (
            &existing_context.document_type,
            &incoming_context.document_type,
        ),
        (
            &existing_context.service_provider,
            &incoming_context.service_provider,
        ),
        (&existing_context.addressee, &incoming_context.addressee),
    ]
    .into_iter()
    .all(|(left, right)| left.is_some() && left == right);
    let subject_matches =
        existing_context.property.as_ref().is_some_and(|property| {
            incoming_context.property.as_deref() == Some(property.as_str())
        }) || existing_context
            .account
            .as_ref()
            .is_some_and(|account| incoming_context.account.as_deref() == Some(account.as_str()));
    core_context_matches && subject_matches
}

fn duplicate_scope_matches(
    expected: &DocumentContextDirection,
    incoming: &DocumentContextDirection,
) -> bool {
    [
        (&expected.document_type, &incoming.document_type),
        (&expected.service_provider, &incoming.service_provider),
        (&expected.addressee, &incoming.addressee),
        (&expected.property, &incoming.property),
        (&expected.account, &incoming.account),
    ]
    .into_iter()
    .all(|(expected, incoming)| expected.is_none() || expected == incoming)
}

fn filing_rule_matches_text(rule: &FilingRule, extracted_text: &str) -> bool {
    let text = comparison_tokens(extracted_text);
    let matches_distinctive = |value: &str| {
        let value = comparison_tokens(value);
        value.len() >= 2
            && contains_token_sequence(&text, &value)
            && !contains_negated_token_sequence(&text, &value)
    };
    let matches_account = |value: &str| {
        let value = comparison_tokens(value);
        !value.is_empty()
            && contains_token_sequence(&text, &value)
            && !contains_negated_token_sequence(&text, &value)
    };
    matches_distinctive(&rule.document_type)
        && matches_distinctive(&rule.service_provider)
        && matches_distinctive(&rule.addressee)
        && rule.property.as_deref().is_none_or(matches_distinctive)
        && rule.account.as_deref().is_none_or(matches_account)
}

fn comparison_tokens(value: &str) -> Vec<String> {
    value
        .split(|character: char| !character.is_alphanumeric())
        .filter(|token| !token.is_empty())
        .map(|token| token.to_ascii_lowercase())
        .collect()
}

fn contains_token_sequence(text: &[String], value: &[String]) -> bool {
    !value.is_empty() && text.windows(value.len()).any(|window| window == value)
}

fn contains_negated_token_sequence(text: &[String], value: &[String]) -> bool {
    (0..=text.len().saturating_sub(value.len())).any(|start| {
        if &text[start..start + value.len()] != value {
            return false;
        }
        (1..=2).any(|distance| {
            start >= distance
                && matches!(
                    text[start - distance].as_str(),
                    "formerly" | "previously" | "from" | "not"
                )
        })
    })
}

fn automatic_filing_decision(
    payload: &DocumentArrivalPayload,
    rule: &FilingRule,
) -> FilingDecisionReview {
    let parent = Path::new(&rule.cabinet_destination)
        .parent()
        .and_then(Path::to_str)
        .unwrap_or_default();
    let generated = propose_filing_decision(payload, parent);
    FilingDecisionReview {
        file_name: generated.file_name,
        cabinet_destination: generated.cabinet_destination,
        confirmed: true,
    }
}

fn clarification_questions(context: &DocumentContextDirection) -> Vec<ClarificationQuestion> {
    let mut questions = Vec::new();
    if !context.document_type_resolved || context.document_type.is_none() {
        questions.push(ClarificationQuestion {
            field: ContextField::DocumentType,
            prompt: "What kind of document is this?".to_owned(),
        });
    }
    if !context.service_provider_resolved || context.service_provider.is_none() {
        questions.push(ClarificationQuestion {
            field: ContextField::ServiceProvider,
            prompt: "Which Service Provider issued this document?".to_owned(),
        });
    } else if context.service_provider_relevance.is_none() {
        questions.push(ClarificationQuestion {
            field: ContextField::ServiceProviderRelevance,
            prompt: format!(
                "How is {} relevant to this Household?",
                context.service_provider.as_deref().unwrap_or_default()
            ),
        });
    }
    if !context.addressee_resolved || context.addressee.is_none() {
        questions.push(ClarificationQuestion {
            field: ContextField::Addressee,
            prompt: "Who is this document addressed to?".to_owned(),
        });
    }
    if !context.property_resolved {
        questions.push(ClarificationQuestion {
            field: ContextField::Property,
            prompt: "Does this document relate to a Household property or address?".to_owned(),
        });
    } else if context.property.is_some() && context.property_relevance.is_none() {
        questions.push(ClarificationQuestion {
            field: ContextField::PropertyRelevance,
            prompt: format!(
                "How is {} relevant to this Household?",
                context.property.as_deref().unwrap_or_default()
            ),
        });
    }
    if !context.account_resolved {
        questions.push(ClarificationQuestion {
            field: ContextField::Account,
            prompt: "Does this document relate to a Household account?".to_owned(),
        });
    }
    if !context.relevant_dates_resolved && !context.relevant_dates.is_empty() {
        questions.push(ClarificationQuestion {
            field: ContextField::RelevantDates,
            prompt: "Which of these dates should identify this document?".to_owned(),
        });
    }
    questions
}

fn unresolved_candidate_fields(context: &DocumentContextDirection) -> Vec<ContextField> {
    let mut fields = clarification_questions(context)
        .into_iter()
        .map(|question| question.field)
        .collect::<Vec<_>>();
    if !context.amount_resolved {
        fields.push(ContextField::Amount);
    }
    if !context.relevant_dates_resolved && context.relevant_dates.is_empty() {
        fields.push(ContextField::RelevantDates);
    }
    fields
}

fn local_evidence_needs_direction_interpretation(
    extracted_text: Option<&str>,
    context: &DocumentContextDirection,
) -> bool {
    let has_content_to_minimise = extracted_text.is_some_and(|text| !text.trim().is_empty());
    has_content_to_minimise
        && clarification_questions(context).iter().any(|question| {
            matches!(
                question.field,
                ContextField::DocumentType
                    | ContextField::ServiceProvider
                    | ContextField::Addressee
                    | ContextField::Property
                    | ContextField::Account
            )
        })
}

fn valid_candidate_value(value: Option<String>) -> Result<Option<String>, ConversationError> {
    let Some(value) = value else {
        return Ok(None);
    };
    let value = value.trim().to_owned();
    if value.is_empty() {
        return Ok(None);
    }
    if value.chars().count() > 512
        || value
            .chars()
            .any(|character| character.is_control() && !character.is_whitespace())
    {
        return Err(ConversationError::InvalidDirectionInterpretation);
    }
    Ok(Some(value))
}

fn valid_candidate_amount(value: Option<String>) -> Result<Option<String>, ConversationError> {
    let Some(value) = valid_candidate_value(value)? else {
        return Ok(None);
    };
    let currency_code = value
        .chars()
        .filter(|character| character.is_ascii_alphabetic())
        .collect::<String>();
    let characters_are_monetary = value.chars().all(|character| {
        character.is_ascii_digit()
            || character.is_ascii_whitespace()
            || matches!(
                character,
                '.' | ',' | '+' | '-' | '(' | ')' | '$' | '£' | '€' | '¥' | '₹'
            )
            || character.is_ascii_uppercase()
    });
    if !value.chars().any(|character| character.is_ascii_digit())
        || !characters_are_monetary
        || (!currency_code.is_empty() && currency_code.len() != 3)
    {
        return Err(ConversationError::InvalidDirectionInterpretation);
    }
    Ok(Some(value))
}

fn valid_candidate_date(value: String) -> Result<Option<String>, ConversationError> {
    let Some(value) = valid_candidate_value(Some(value))? else {
        return Ok(None);
    };
    if !valid_iso_calendar_date(&value) {
        return Err(ConversationError::InvalidDirectionInterpretation);
    }
    Ok(Some(value))
}

fn valid_iso_calendar_date(value: &str) -> bool {
    if value.len() != 10
        || value.as_bytes().get(4) != Some(&b'-')
        || value.as_bytes().get(7) != Some(&b'-')
    {
        return false;
    }
    let Ok(year) = value[0..4].parse::<u32>() else {
        return false;
    };
    let Ok(month) = value[5..7].parse::<u32>() else {
        return false;
    };
    let Ok(day) = value[8..10].parse::<u32>() else {
        return false;
    };
    let leap_year =
        year.is_multiple_of(4) && (!year.is_multiple_of(100) || year.is_multiple_of(400));
    let days_in_month = match month {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        4 | 6 | 9 | 11 => 30,
        2 if leap_year => 29,
        2 => 28,
        _ => return false,
    };
    (1..=days_in_month).contains(&day)
}

fn propose_filing_decision(
    payload: &DocumentArrivalPayload,
    cabinet_section: &str,
) -> FilingDecisionReview {
    propose_filing_decision_from_context(
        &payload.original_name,
        &payload.context_direction,
        cabinet_section,
    )
}

fn propose_filing_decision_from_context(
    original_name: &str,
    context: &DocumentContextDirection,
    cabinet_section: &str,
) -> FilingDecisionReview {
    let date = context
        .relevant_dates
        .first()
        .map(String::as_str)
        .unwrap_or("Undated");
    let year = date
        .get(..4)
        .filter(|year| year.chars().all(|character| character.is_ascii_digit()))
        .unwrap_or("Undated");
    let service_provider = context
        .service_provider
        .as_deref()
        .unwrap_or("Unknown provider");
    let document_type = context.document_type.as_deref().unwrap_or("Document");
    let addressee = context.addressee.as_deref().unwrap_or("Household");
    let extension = Path::new(original_name)
        .extension()
        .and_then(|extension| extension.to_str())
        .map(str::to_ascii_lowercase)
        .unwrap_or_else(|| "pdf".to_owned());
    let file_name = format!(
        "{} - {} - {} - {}.{}",
        safe_generated_component(date),
        safe_generated_component(service_provider),
        safe_generated_component(document_type),
        safe_generated_component(addressee),
        extension
    );
    let subject = context
        .property
        .as_deref()
        .or(context.account.as_deref())
        .unwrap_or(addressee);
    let cabinet_destination = [
        safe_component(cabinet_section),
        safe_generated_component(subject),
        safe_generated_component(service_provider),
        safe_generated_component(year),
        file_name.clone(),
    ]
    .join("/");
    FilingDecisionReview {
        file_name,
        cabinet_destination,
        confirmed: false,
    }
}

fn safe_component(value: &str) -> String {
    let sanitized = value
        .chars()
        .map(|character| {
            if character.is_control() || "<>:\"/\\|?*".contains(character) {
                '-'
            } else {
                character
            }
        })
        .collect::<String>();
    let sanitized = sanitized.trim().trim_matches('.').trim();
    let sanitized = if sanitized.is_empty() {
        "Unknown"
    } else {
        sanitized
    };
    if reserved_windows_name(sanitized) {
        match sanitized.rsplit_once('.') {
            Some((stem, extension)) => format!("{stem}_.{extension}"),
            None => format!("{sanitized}_"),
        }
    } else {
        sanitized.to_owned()
    }
}

fn safe_generated_component(value: &str) -> String {
    let safe = safe_component(value);
    if safe.len() <= 48 {
        return safe;
    }
    let mut shortened = String::new();
    for character in safe.chars() {
        if shortened.len() + character.len_utf8() > 48 {
            break;
        }
        shortened.push(character);
    }
    safe_component(&shortened)
}

fn valid_file_name(file_name: &str, original_name: &str) -> bool {
    if file_name.is_empty()
        || file_name == "."
        || file_name == ".."
        || !valid_path_component(file_name)
    {
        return false;
    }
    let extension = Path::new(file_name)
        .extension()
        .and_then(|extension| extension.to_str());
    let original_extension = Path::new(original_name)
        .extension()
        .and_then(|extension| extension.to_str());
    extension
        .zip(original_extension)
        .is_some_and(|(left, right)| left.eq_ignore_ascii_case(right))
}

fn valid_cabinet_destination(cabinet_destination: &str, file_name: &str) -> bool {
    if cabinet_destination.is_empty()
        || cabinet_destination.starts_with(['/', '\\'])
        || cabinet_destination.contains('\\')
    {
        return false;
    }
    let components = cabinet_destination.split('/').collect::<Vec<_>>();
    components.len() >= 2
        && !components[0].eq_ignore_ascii_case("Incoming")
        && components
            .last()
            .is_some_and(|component| *component == file_name)
        && components.iter().all(|component| {
            !component.is_empty()
                && *component != "."
                && *component != ".."
                && valid_path_component(component)
        })
}

fn valid_rule_directory(directory: &str) -> bool {
    if directory.is_empty() || directory.starts_with(['/', '\\']) || directory.contains('\\') {
        return false;
    }
    let components = directory.split('/').collect::<Vec<_>>();
    !components.is_empty()
        && !components[0].eq_ignore_ascii_case("Incoming")
        && components.iter().all(|component| {
            !component.is_empty()
                && *component != "."
                && *component != ".."
                && valid_path_component(component)
        })
}

fn valid_path_component(component: &str) -> bool {
    component.len() <= 240
        && safe_component(component) == component
        && !reserved_windows_name(component)
}

fn reserved_windows_name(component: &str) -> bool {
    let stem = component.split('.').next().unwrap_or_default();
    matches!(
        stem.to_ascii_uppercase().as_str(),
        "CON"
            | "CONIN$"
            | "CONOUT$"
            | "CLOCK$"
            | "PRN"
            | "AUX"
            | "NUL"
            | "COM1"
            | "COM2"
            | "COM3"
            | "COM4"
            | "COM5"
            | "COM6"
            | "COM7"
            | "COM8"
            | "COM9"
            | "COM¹"
            | "COM²"
            | "COM³"
            | "LPT1"
            | "LPT2"
            | "LPT3"
            | "LPT4"
            | "LPT5"
            | "LPT6"
            | "LPT7"
            | "LPT8"
            | "LPT9"
            | "LPT¹"
            | "LPT²"
            | "LPT³"
    )
}

impl DocumentContextDirection {
    fn normalized(self) -> Self {
        let document_type = non_empty(self.document_type);
        let service_provider = non_empty(self.service_provider);
        let addressee = non_empty(self.addressee);
        let property = non_empty(self.property);
        let account = non_empty(self.account);
        let amount = non_empty(self.amount);
        Self {
            document_type,
            document_type_resolved: self.document_type_resolved,
            service_provider_relevance: normalized_relevance(
                self.service_provider_relevance,
                service_provider.as_deref(),
            ),
            service_provider,
            service_provider_resolved: self.service_provider_resolved,
            addressee,
            addressee_resolved: self.addressee_resolved,
            property_relevance: normalized_relevance(self.property_relevance, property.as_deref()),
            property,
            property_resolved: self.property_resolved,
            account,
            account_resolved: self.account_resolved,
            amount,
            amount_resolved: self.amount_resolved,
            relevant_dates: self
                .relevant_dates
                .into_iter()
                .filter_map(|date| non_empty(Some(date)))
                .collect(),
            relevant_dates_resolved: self.relevant_dates_resolved,
        }
    }
}

fn normalized_relevance(
    relevance: Option<ContextRelevanceDirection>,
    current_subject: Option<&str>,
) -> Option<ContextRelevanceDirection> {
    let relevance = relevance?;
    let subject = non_empty(Some(relevance.subject))?;
    let explanation = non_empty(Some(relevance.explanation))?;
    (current_subject == Some(subject.as_str())).then_some(ContextRelevanceDirection {
        subject,
        explanation,
    })
}

fn non_empty(value: Option<String>) -> Option<String> {
    value.and_then(|value| {
        let value = value.trim().to_owned();
        (!value.is_empty()).then_some(value)
    })
}

fn review_field(value: &Option<String>, resolved: bool) -> ReviewField {
    ReviewField {
        value: value.clone(),
        confidence_state: if resolved {
            ConfidenceState::Confirmed
        } else if value.is_some() {
            ConfidenceState::LooksRight
        } else {
            ConfidenceState::Unknown
        },
    }
}

fn media_type_label(media_type: &str) -> &str {
    match media_type {
        "application/pdf" => "PDF",
        "image/jpeg" => "JPG",
        "image/png" => "PNG",
        _ => "Unknown",
    }
}
