//! Application service joining Document Handling to the Luna-owned
//! Intelligence Gateway without exposing infrastructure details to the domain.

use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::{
    CloudConsentDecision, CloudIntelligenceStore, ConversationError, ConversationStore,
    CredentialVault, DocumentContentExcerpt, DocumentProcessingState, IntelligenceCapability,
    IntelligenceEvidence, IntelligenceExecutionConstraints, IntelligenceFailure,
    IntelligenceRequest, IntelligenceResponseSchema, IntelligenceResult, IntelligenceSelection,
};

const MAX_DOCUMENT_EXCERPT_CHARS: usize = 4_000;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CloudAssistanceResolution {
    pub result: Option<IntelligenceResult>,
    pub processing_state: DocumentProcessingState,
}

#[derive(Debug, Error)]
pub enum DocumentIntelligenceError {
    #[error("{0}")]
    DocumentHandling(#[from] ConversationError),
    #[error("{0}")]
    Intelligence(#[from] IntelligenceFailure),
}

#[derive(Clone)]
pub struct DocumentIntelligenceService<V: CredentialVault> {
    conversations: ConversationStore<V>,
    intelligence: CloudIntelligenceStore<V>,
}

impl<V: CredentialVault> DocumentIntelligenceService<V> {
    pub fn new(
        conversations: ConversationStore<V>,
        intelligence: CloudIntelligenceStore<V>,
    ) -> Self {
        Self {
            conversations,
            intelligence,
        }
    }

    #[allow(clippy::too_many_arguments)]
    pub fn evaluate_document(
        &self,
        household_id: &str,
        arrival_id: i64,
        selection: IntelligenceSelection,
        consent: CloudConsentDecision,
        granted_by: &str,
        existing_consent_grant_id: Option<i64>,
    ) -> Result<CloudAssistanceResolution, DocumentIntelligenceError> {
        let arrival = self
            .conversations
            .list_document_arrivals(household_id)?
            .into_iter()
            .find(|arrival| arrival.id == arrival_id)
            .ok_or(ConversationError::NotFound)?;
        let request = authorised_request(&arrival, &selection)?;
        let provider_id = selection.provider_id.clone();
        let model_id = selection.model_id.clone();

        if consent == CloudConsentDecision::KeepLocal {
            match self.intelligence.evaluate_document(
                household_id,
                selection,
                request,
                consent,
                granted_by,
                existing_consent_grant_id,
            ) {
                Err(IntelligenceFailure::LocalOnlyPolicy) => {
                    let arrival = self
                        .conversations
                        .keep_document_local(household_id, arrival_id)?;
                    self.conversations.record_cloud_assistance_event(
                        household_id,
                        arrival_id,
                        "Member chose Keep local; no document information was sent to an Intelligence Provider.",
                    )?;
                    return Ok(CloudAssistanceResolution {
                        result: None,
                        processing_state: arrival.processing_state,
                    });
                }
                Err(failure) => return Err(failure.into()),
                Ok(_) => return Err(IntelligenceFailure::InvalidStructuredResult.into()),
            }
        }

        self.conversations
            .begin_cloud_assistance(household_id, arrival_id)?;
        match self.intelligence.evaluate_document(
            household_id,
            selection,
            request,
            consent,
            granted_by,
            existing_consent_grant_id,
        ) {
            Ok(mut result) => {
                if let Some(candidate) = result.candidate_direction.take() {
                    match self.conversations.validate_candidate_direction(
                        household_id,
                        arrival_id,
                        candidate,
                    ) {
                        Ok(candidate) => result.candidate_direction = Some(candidate),
                        Err(error) => {
                            self.intelligence.record_candidate_validation_failure(
                                household_id,
                                &result.request_id,
                            )?;
                            self.conversations
                                .wait_for_cloud_assistance(household_id, arrival_id)?;
                            self.conversations.record_cloud_assistance_event(
                                household_id,
                                arrival_id,
                                &format!(
                                    "{provider_id} {model_id} returned candidate Evidence that failed Luna validation; the Document remains waiting for retry."
                                ),
                            )?;
                            return Err(error.into());
                        }
                    }
                }
                let arrival = self
                    .conversations
                    .complete_cloud_assistance(household_id, arrival_id)?;
                self.conversations.record_cloud_assistance_event(
                    household_id,
                    arrival_id,
                    &format!(
                        "{provider_id} {model_id} returned untrusted candidate Evidence under {} consent; Member Direction is still required.",
                        consent_label(consent),
                    ),
                )?;
                Ok(CloudAssistanceResolution {
                    result: Some(result),
                    processing_state: arrival.processing_state,
                })
            }
            Err(failure) => {
                self.conversations
                    .wait_for_cloud_assistance(household_id, arrival_id)?;
                self.conversations.record_cloud_assistance_event(
                    household_id,
                    arrival_id,
                    &format!(
                        "{provider_id} {model_id} Cloud Assistance failed; the Document remains waiting for the same route."
                    ),
                )?;
                Err(failure.into())
            }
        }
    }
}

fn consent_label(consent: CloudConsentDecision) -> &'static str {
    match consent {
        CloudConsentDecision::AllowOnce => "one-time",
        CloudConsentDecision::AllowForScope => "reusable scoped",
        CloudConsentDecision::UseExistingScope => "existing scoped",
        CloudConsentDecision::KeepLocal => "local-only",
    }
}

fn authorised_request(
    arrival: &crate::DocumentArrival,
    selection: &IntelligenceSelection,
) -> Result<IntelligenceRequest, IntelligenceFailure> {
    let context = &arrival.review_card.context;
    let mut allowed_fields = [
        ("documentType", &context.document_type),
        ("serviceProvider", &context.service_provider),
        ("addressee", &context.addressee),
        ("property", &context.property),
        ("account", &context.account),
        ("amount", &context.amount),
    ]
    .into_iter()
    .filter(|(_, field)| field.confidence_state != crate::ConfidenceState::Confirmed)
    .map(|(field, _)| field.to_owned())
    .collect::<Vec<_>>();
    if context.relevant_dates.is_empty()
        || context
            .relevant_dates
            .iter()
            .any(|field| field.confidence_state != crate::ConfidenceState::Confirmed)
    {
        allowed_fields.push("relevantDates".to_owned());
    }
    if allowed_fields.is_empty() {
        return Err(IntelligenceFailure::UnsupportedCapability);
    }

    let mut evidence = vec![IntelligenceEvidence {
        field: "mediaType".to_owned(),
        value: arrival.media_type.clone(),
        source: "Local Inspection".to_owned(),
    }];
    for field in &allowed_fields {
        if let Some(value) = review_value(arrival, field) {
            evidence.push(IntelligenceEvidence {
                field: field.clone(),
                value,
                source: "Local Inspection".to_owned(),
            });
        }
    }
    let content_excerpts = arrival
        .extracted_text
        .as_deref()
        .map(str::trim)
        .filter(|text| !text.is_empty())
        .map(|text| {
            vec![DocumentContentExcerpt {
                source: "locally extracted text".to_owned(),
                text: text.chars().take(MAX_DOCUMENT_EXCERPT_CHARS).collect(),
            }]
        })
        .unwrap_or_default();

    Ok(IntelligenceRequest {
        request_id: format!("intelligence-{}-{}", arrival.id, request_timestamp()),
        document_arrival_id: format!("arrival-{}", arrival.id),
        capability: IntelligenceCapability::DirectionInterpretation,
        provider_id: selection.provider_id.clone(),
        model_id: selection.model_id.clone(),
        evidence,
        content_excerpts,
        expected_response: IntelligenceResponseSchema {
            allowed_fields,
            allow_candidate_direction: true,
        },
        consent_grant_id: None,
        constraints: IntelligenceExecutionConstraints {
            timeout_ms: 30_000,
            max_output_tokens: 512,
        },
    })
}

fn review_value(arrival: &crate::DocumentArrival, field: &str) -> Option<String> {
    let context = &arrival.review_card.context;
    match field {
        "documentType" => context.document_type.value.clone(),
        "serviceProvider" => context.service_provider.value.clone(),
        "addressee" => context.addressee.value.clone(),
        "property" => context.property.value.clone(),
        "account" => context.account.value.clone(),
        "amount" => context.amount.value.clone(),
        "relevantDates" => {
            let dates = context
                .relevant_dates
                .iter()
                .filter_map(|date| date.value.as_deref())
                .collect::<Vec<_>>()
                .join(", ");
            (!dates.is_empty()).then_some(dates)
        }
        _ => None,
    }
}

fn request_timestamp() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_nanos())
        .unwrap_or_default()
}
