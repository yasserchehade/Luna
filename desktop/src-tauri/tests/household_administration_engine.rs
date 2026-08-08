use std::{
    collections::BTreeMap,
    io::Cursor,
    sync::{Arc, Mutex},
};

use base64::{engine::general_purpose::STANDARD as BASE64, Engine};
use image::{DynamicImage, ImageBuffer, ImageFormat, Rgb};
use luna_core::{
    ActionApproval, ActionExecution, AvailableHouseholdTool, ConversationPort,
    ConversationPortError, FixedHouseholdAdministrationClock, HandleHouseholdAdministrationTurn,
    HouseholdActionProposal, HouseholdAdministrationEngine, HouseholdAdministrationFailureCategory,
    HouseholdAdministrationMessage, HouseholdAdministrationReasoning,
    HouseholdAdministrationRequest, HouseholdAdministrationSource, HouseholdContextItem,
    HouseholdWork, HouseholdWorkKind, HouseholdWorkOperation, HouseholdWorkPort,
    HouseholdWorkPortError, HouseholdWorkProposal, HouseholdWorkStatus, IntelligenceUsage,
    OpenAiHouseholdAdministrationReasoningAdapter, ProposedAction, ProposedActionKind,
    ReasoningPortError, SourcePort, SourcePortError, UntrustedHouseholdAdministrationResult,
    WorkFact, WorkFactCertainty, WorkFactKey,
};

#[derive(Default)]
struct MemoryConversationPort {
    messages: Mutex<Vec<HouseholdAdministrationMessage>>,
}

impl ConversationPort for MemoryConversationPort {
    fn recent_messages(
        &self,
        _household_id: &str,
        _conversation_id: i64,
        limit: usize,
    ) -> Result<Vec<HouseholdAdministrationMessage>, ConversationPortError> {
        let messages = self.messages.lock().expect("conversation lock");
        Ok(messages
            .iter()
            .rev()
            .take(limit)
            .cloned()
            .collect::<Vec<_>>()
            .into_iter()
            .rev()
            .collect())
    }

    fn append_member_message(
        &self,
        _household_id: &str,
        _conversation_id: i64,
        body: &str,
    ) -> Result<(), ConversationPortError> {
        self.messages
            .lock()
            .expect("conversation lock")
            .push(HouseholdAdministrationMessage {
                author: "member".to_owned(),
                body: body.to_owned(),
            });
        Ok(())
    }

    fn append_luna_message(
        &self,
        _household_id: &str,
        _conversation_id: i64,
        body: &str,
        _source_reference: Option<&str>,
    ) -> Result<(), ConversationPortError> {
        self.messages
            .lock()
            .expect("conversation lock")
            .push(HouseholdAdministrationMessage {
                author: "luna".to_owned(),
                body: body.to_owned(),
            });
        Ok(())
    }
}

struct MemoryHouseholdWorkPort {
    works: Mutex<Vec<HouseholdWork>>,
    fail_save: bool,
}

impl Default for MemoryHouseholdWorkPort {
    fn default() -> Self {
        Self {
            works: Mutex::new(Vec::new()),
            fail_save: false,
        }
    }
}

impl HouseholdWorkPort for MemoryHouseholdWorkPort {
    fn list(&self, _household_id: &str) -> Result<Vec<HouseholdWork>, HouseholdWorkPortError> {
        Ok(self.works.lock().expect("work lock").clone())
    }

    fn save(
        &self,
        _household_id: &str,
        work: &HouseholdWork,
    ) -> Result<(), HouseholdWorkPortError> {
        if self.fail_save {
            return Err(HouseholdWorkPortError::Unavailable);
        }
        let mut works = self.works.lock().expect("work lock");
        if let Some(existing) = works.iter_mut().find(|existing| existing.id == work.id) {
            *existing = work.clone();
        } else {
            works.push(work.clone());
        }
        Ok(())
    }
}

struct FixtureSourcePort {
    source: HouseholdAdministrationSource,
}

impl SourcePort for FixtureSourcePort {
    fn load(
        &self,
        _household_id: &str,
        source_reference: &str,
    ) -> Result<HouseholdAdministrationSource, SourcePortError> {
        if self.source.reference != source_reference {
            return Err(SourcePortError::NotFound);
        }
        Ok(self.source.clone())
    }
}

struct FixtureReasoningPort {
    result: Result<UntrustedHouseholdAdministrationResult, ReasoningPortError>,
    request: Arc<Mutex<Option<HouseholdAdministrationRequest>>>,
}

struct CapturingReasoningPort<R> {
    inner: R,
    result: Arc<Mutex<Option<Result<UntrustedHouseholdAdministrationResult, ReasoningPortError>>>>,
}

impl<R: HouseholdAdministrationReasoning> HouseholdAdministrationReasoning
    for CapturingReasoningPort<R>
{
    fn reason(
        &self,
        request: &HouseholdAdministrationRequest,
    ) -> Result<UntrustedHouseholdAdministrationResult, ReasoningPortError> {
        let result = self.inner.reason(request);
        *self.result.lock().expect("reasoning result lock") = Some(result.clone());
        result
    }
}

impl HouseholdAdministrationReasoning for FixtureReasoningPort {
    fn reason(
        &self,
        request: &HouseholdAdministrationRequest,
    ) -> Result<UntrustedHouseholdAdministrationResult, ReasoningPortError> {
        *self.request.lock().expect("request lock") = Some(request.clone());
        self.result.clone()
    }
}

fn fixture_source(reference: &str, media_type: &str, text: &str) -> FixtureSourcePort {
    FixtureSourcePort {
        source: HouseholdAdministrationSource {
            reference: reference.to_owned(),
            filename: if media_type == "application/pdf" {
                "household-source.pdf"
            } else {
                "household-source.png"
            }
            .to_owned(),
            media_type: media_type.to_owned(),
            original_base64: "c3ludGhldGljLWZpeHR1cmU=".to_owned(),
            extracted_text: Some(text.to_owned()),
            original_size_bytes: 17,
            extracted_text_truncated: false,
        },
    }
}

fn existing_bill(status: HouseholdWorkStatus) -> HouseholdWork {
    let mut work = HouseholdWork::new(
        "work-1",
        "rivera-household",
        HouseholdWorkKind::Bill,
        "Pay the AGL electricity bill",
        "2026-08-01T10:00:00Z",
    );
    work.status = status;
    work.source_refs.push("document-41".to_owned());
    work.facts = vec![
        WorkFact {
            key: WorkFactKey::Provider,
            value: "AGL".to_owned(),
            evidence_refs: vec!["document-41".to_owned()],
            certainty: WorkFactCertainty::Confirmed,
        },
        WorkFact {
            key: WorkFactKey::Amount,
            value: "$184.72".to_owned(),
            evidence_refs: vec!["document-41".to_owned()],
            certainty: WorkFactCertainty::Confirmed,
        },
        WorkFact {
            key: WorkFactKey::Account,
            value: "7788".to_owned(),
            evidence_refs: vec!["document-41".to_owned()],
            certainty: WorkFactCertainty::Confirmed,
        },
    ];
    work.proposed_actions.push(ProposedAction {
        id: "work-1-action-1".to_owned(),
        kind: ProposedActionKind::Reminder,
        summary: "Remind the household before the due date.".to_owned(),
        arguments: BTreeMap::new(),
        approval: ActionApproval::Required,
        execution: ActionExecution::NotStarted,
    });
    work
}

fn turn_input(message: &str, request_id: &str) -> HandleHouseholdAdministrationTurn {
    HandleHouseholdAdministrationTurn {
        household_id: "rivera-household".to_owned(),
        conversation_id: 7,
        member_message: message.to_owned(),
        source_reference: Some("document-41".to_owned()),
        active_work_reference: None,
        authorised_household_context: vec![HouseholdContextItem {
            category: "property".to_owned(),
            value: "12 Seabreeze Avenue".to_owned(),
            source_reference: "household-property-seabreeze".to_owned(),
        }],
        available_actions: vec![AvailableHouseholdTool {
            name: "reminder".to_owned(),
            description: "Propose a reminder without scheduling it.".to_owned(),
        }],
        authorised_actor: "member-1".to_owned(),
        request_id: request_id.to_owned(),
    }
}

fn no_work_proposal() -> HouseholdWorkProposal {
    HouseholdWorkProposal {
        operation: HouseholdWorkOperation::None,
        work_id: None,
        kind: None,
        summary: None,
        status: None,
        facts: Vec::new(),
        due_at: None,
        urgency: None,
    }
}

fn provider_result(
    request_id: &str,
    reply: &str,
    work: HouseholdWorkProposal,
) -> UntrustedHouseholdAdministrationResult {
    UntrustedHouseholdAdministrationResult {
        request_id: request_id.to_owned(),
        provider_id: "openai".to_owned(),
        model_id: "gpt-5.6-luna".to_owned(),
        reply: reply.to_owned(),
        work,
        clarification: None,
        proposed_actions: Vec::new(),
        usage: IntelligenceUsage::default(),
    }
}

#[test]
fn clear_pdf_like_source_creates_household_work_without_desktop_or_repeated_questions() {
    let conversations = MemoryConversationPort::default();
    let work = MemoryHouseholdWorkPort::default();
    let source = FixtureSourcePort {
        source: HouseholdAdministrationSource {
            reference: "document-41".to_owned(),
            filename: "electricity-bill.pdf".to_owned(),
            media_type: "application/pdf".to_owned(),
            original_base64: "cGRm".to_owned(),
            extracted_text: Some(
                "AGL electricity bill for 12 Seabreeze Avenue. Account 7788. $184.72 due 15 August 2026."
                    .to_owned(),
            ),
            original_size_bytes: 3,
            extracted_text_truncated: false,
        },
    };
    let captured_request = Arc::new(Mutex::new(None));
    let reasoning = FixtureReasoningPort {
        request: captured_request.clone(),
        result: Ok(UntrustedHouseholdAdministrationResult {
            request_id: "request-clear-pdf".to_owned(),
            provider_id: "openai".to_owned(),
            model_id: "gpt-5.6-luna".to_owned(),
            reply: "I found the AGL bill for the Seabreeze property. It is $184.72 and due 15 August. I can remind you before it is due.".to_owned(),
            work: HouseholdWorkProposal {
                operation: HouseholdWorkOperation::Create,
                work_id: None,
                kind: Some(HouseholdWorkKind::Bill),
                summary: Some("Pay the AGL electricity bill".to_owned()),
                status: Some(HouseholdWorkStatus::AwaitingApproval),
                facts: vec![
                    WorkFact { key: WorkFactKey::Provider, value: "AGL".to_owned(), evidence_refs: vec!["document-41".to_owned()], certainty: WorkFactCertainty::Confirmed },
                    WorkFact { key: WorkFactKey::Property, value: "12 Seabreeze Avenue".to_owned(), evidence_refs: vec!["document-41".to_owned()], certainty: WorkFactCertainty::Confirmed },
                    WorkFact { key: WorkFactKey::Account, value: "7788".to_owned(), evidence_refs: vec!["document-41".to_owned()], certainty: WorkFactCertainty::Confirmed },
                    WorkFact { key: WorkFactKey::Amount, value: "$184.72".to_owned(), evidence_refs: vec!["document-41".to_owned()], certainty: WorkFactCertainty::Confirmed },
                    WorkFact { key: WorkFactKey::DueDate, value: "2026-08-15".to_owned(), evidence_refs: vec!["document-41".to_owned()], certainty: WorkFactCertainty::Confirmed },
                ],
                due_at: Some("2026-08-15".to_owned()),
                urgency: Some("normal".to_owned()),
            },
            clarification: None,
            proposed_actions: vec![HouseholdActionProposal {
                kind: ProposedActionKind::Reminder,
                summary: "Remind the household before the bill is due.".to_owned(),
                arguments: Default::default(),
                approval_required: true,
            }],
            usage: IntelligenceUsage::default(),
        }),
    };
    let clock = FixedHouseholdAdministrationClock::new("2026-08-05T10:00:00Z");
    let engine =
        HouseholdAdministrationEngine::new(&conversations, &work, &source, &reasoning, &clock);

    let outcome = engine
        .handle_turn(HandleHouseholdAdministrationTurn {
            household_id: "rivera-household".to_owned(),
            conversation_id: 7,
            member_message: "Take care of this.".to_owned(),
            source_reference: Some("document-41".to_owned()),
            active_work_reference: None,
            authorised_household_context: vec![HouseholdContextItem {
                category: "property".to_owned(),
                value: "12 Seabreeze Avenue".to_owned(),
                source_reference: "household-property-seabreeze".to_owned(),
            }],
            available_actions: vec![AvailableHouseholdTool {
                name: "reminder".to_owned(),
                description: "Propose a reminder without scheduling it.".to_owned(),
            }],
            authorised_actor: "member-1".to_owned(),
            request_id: "request-clear-pdf".to_owned(),
        })
        .expect("handle the Household Administration turn");

    let created = outcome.work.expect("created Household Work");
    assert_eq!(created.status, HouseholdWorkStatus::AwaitingApproval);
    assert_eq!(created.facts.len(), 5);
    assert_eq!(work.works.lock().expect("work lock").as_slice(), &[created]);
    assert!(!outcome.message.contains('?'));

    let request = captured_request
        .lock()
        .expect("request lock")
        .clone()
        .expect("captured reasoning request");
    assert_eq!(request.current_message, "Take care of this.");
    assert_eq!(request.relevant_conversation.len(), 1);
    assert_eq!(
        request.source.expect("bounded source").reference,
        "document-41"
    );
}

#[test]
fn clarification_reuses_existing_work_and_updates_only_the_answered_fact() {
    let conversations = MemoryConversationPort::default();
    let original = existing_bill(HouseholdWorkStatus::NeedsClarification);
    let work = MemoryHouseholdWorkPort {
        works: Mutex::new(vec![original.clone()]),
        fail_save: false,
    };
    let source = fixture_source(
        "document-41",
        "application/pdf",
        "AGL bill for $184.72; the property is not printed clearly.",
    );
    let captured = Arc::new(Mutex::new(None));
    let reasoning = FixtureReasoningPort {
        request: captured,
        result: Ok(provider_result(
            "request-clarification",
            "Thanks — I’ve linked the bill to the rental property.",
            HouseholdWorkProposal {
                operation: HouseholdWorkOperation::Create,
                work_id: None,
                kind: None,
                summary: None,
                status: Some(HouseholdWorkStatus::Active),
                facts: vec![WorkFact {
                    key: WorkFactKey::Property,
                    value: "The rental property".to_owned(),
                    evidence_refs: vec!["conversation-member".to_owned()],
                    certainty: WorkFactCertainty::Confirmed,
                }],
                due_at: None,
                urgency: None,
            },
        )),
    };
    let clock = FixedHouseholdAdministrationClock::new("2026-08-05T10:01:00Z");
    let engine =
        HouseholdAdministrationEngine::new(&conversations, &work, &source, &reasoning, &clock);

    let outcome = engine
        .handle_turn(turn_input("The rental property.", "request-clarification"))
        .expect("apply the clarification");

    let updated = outcome.work.expect("updated Household Work");
    assert_eq!(updated.id, original.id);
    assert_eq!(updated.status, HouseholdWorkStatus::Active);
    assert_eq!(updated.proposed_actions, original.proposed_actions);
    assert!(updated
        .facts
        .iter()
        .any(|fact| { fact.key == WorkFactKey::Property && fact.value == "The rental property" }));
    assert!(updated
        .facts
        .iter()
        .any(|fact| fact.key == WorkFactKey::Amount && fact.value == "$184.72"));
    assert_eq!(work.works.lock().expect("work lock").len(), 1);
    assert_eq!(outcome.clarification, None);
}

#[test]
fn correction_targets_existing_work_and_preserves_unrelated_facts_and_proposals() {
    let conversations = MemoryConversationPort::default();
    let mut original = existing_bill(HouseholdWorkStatus::Active);
    original.facts.push(WorkFact {
        key: WorkFactKey::Property,
        value: "Our home".to_owned(),
        evidence_refs: vec!["document-41".to_owned()],
        certainty: WorkFactCertainty::Likely,
    });
    let work = MemoryHouseholdWorkPort {
        works: Mutex::new(vec![original.clone()]),
        fail_save: false,
    };
    let source = fixture_source("document-41", "application/pdf", "AGL electricity bill");
    let reasoning = FixtureReasoningPort {
        request: Arc::new(Mutex::new(None)),
        result: Ok(provider_result(
            "request-correction",
            "I’ve corrected the bill to the rental property and kept the other details.",
            HouseholdWorkProposal {
                operation: HouseholdWorkOperation::Update,
                work_id: Some("work-1".to_owned()),
                kind: None,
                summary: None,
                status: Some(HouseholdWorkStatus::Active),
                facts: vec![WorkFact {
                    key: WorkFactKey::Property,
                    value: "The rental property".to_owned(),
                    evidence_refs: vec!["conversation-member".to_owned()],
                    certainty: WorkFactCertainty::Confirmed,
                }],
                due_at: None,
                urgency: None,
            },
        )),
    };
    let clock = FixedHouseholdAdministrationClock::new("2026-08-05T10:02:00Z");
    let engine =
        HouseholdAdministrationEngine::new(&conversations, &work, &source, &reasoning, &clock);

    let outcome = engine
        .handle_turn(turn_input(
            "That bill is for the rental property, not our home.",
            "request-correction",
        ))
        .expect("apply the correction");

    let updated = outcome.work.expect("corrected Household Work");
    assert_eq!(updated.id, original.id);
    assert_eq!(updated.proposed_actions, original.proposed_actions);
    assert!(updated
        .facts
        .iter()
        .any(|fact| fact.key == WorkFactKey::Provider && fact.value == "AGL"));
    assert!(updated
        .facts
        .iter()
        .any(|fact| fact.key == WorkFactKey::Account && fact.value == "7788"));
    assert!(updated
        .facts
        .iter()
        .any(|fact| { fact.key == WorkFactKey::Property && fact.value == "The rental property" }));
    assert!(outcome.message.starts_with("I’ve corrected"));
}

#[test]
fn read_only_question_returns_an_answer_without_mutating_household_work() {
    let conversations = MemoryConversationPort::default();
    let mut original = existing_bill(HouseholdWorkStatus::AwaitingApproval);
    original
        .audit_events
        .push("Household Work created".to_owned());
    let work = MemoryHouseholdWorkPort {
        works: Mutex::new(vec![original.clone()]),
        fail_save: false,
    };
    let source = fixture_source("document-41", "application/pdf", "AGL account number 7788");
    let mut result = provider_result(
        "request-read-only",
        "The account number is 7788.",
        HouseholdWorkProposal {
            operation: HouseholdWorkOperation::Update,
            work_id: Some("work-1".to_owned()),
            kind: Some(HouseholdWorkKind::Other),
            summary: Some("Untrusted replacement".to_owned()),
            status: Some(HouseholdWorkStatus::Active),
            facts: vec![WorkFact {
                key: WorkFactKey::Account,
                value: "9999".to_owned(),
                evidence_refs: vec!["document-41".to_owned()],
                certainty: WorkFactCertainty::Likely,
            }],
            due_at: None,
            urgency: None,
        },
    );
    result.proposed_actions.push(HouseholdActionProposal {
        kind: ProposedActionKind::DraftReply,
        summary: "Replace the existing action".to_owned(),
        arguments: BTreeMap::new(),
        approval_required: false,
    });
    let reasoning = FixtureReasoningPort {
        request: Arc::new(Mutex::new(None)),
        result: Ok(result),
    };
    let clock = FixedHouseholdAdministrationClock::new("2026-08-05T10:03:00Z");
    let engine =
        HouseholdAdministrationEngine::new(&conversations, &work, &source, &reasoning, &clock);

    let outcome = engine
        .handle_turn(turn_input(
            "What is the account number?",
            "request-read-only",
        ))
        .expect("answer the read-only question");

    assert_eq!(outcome.message, "The account number is 7788.");
    assert_eq!(outcome.work.as_ref(), Some(&original));
    assert_eq!(
        work.works.lock().expect("work lock").as_slice(),
        &[original]
    );
}

fn assert_terminal_member_direction(
    message: &str,
    request_id: &str,
    expected_status: HouseholdWorkStatus,
) {
    let conversations = MemoryConversationPort::default();
    let original = existing_bill(HouseholdWorkStatus::AwaitingApproval);
    let work = MemoryHouseholdWorkPort {
        works: Mutex::new(vec![original]),
        fail_save: false,
    };
    let source = fixture_source("document-41", "application/pdf", "AGL bill account 7788");
    let reasoning = FixtureReasoningPort {
        request: Arc::new(Mutex::new(None)),
        result: Ok(provider_result(
            request_id,
            "The model proposed no state change.",
            no_work_proposal(),
        )),
    };
    let clock = FixedHouseholdAdministrationClock::new("2026-08-05T10:04:00Z");
    let engine =
        HouseholdAdministrationEngine::new(&conversations, &work, &source, &reasoning, &clock);

    let outcome = engine
        .handle_turn(turn_input(message, request_id))
        .expect("apply validated member direction");
    let updated = outcome.work.expect("terminal Household Work");
    assert_eq!(updated.status, expected_status);
    assert_eq!(updated.closed_at.as_deref(), Some("2026-08-05T10:04:00Z"));
    assert!(updated
        .audit_events
        .iter()
        .any(|event| event == "Household Work updated"));
}

#[test]
fn explicit_member_payment_completes_work_through_luna_owned_validation() {
    assert_terminal_member_direction(
        "I already paid that.",
        "request-completion",
        HouseholdWorkStatus::Completed,
    );
}

#[test]
fn explicit_member_dismissal_closes_work_through_luna_owned_validation() {
    assert_terminal_member_direction(
        "We do not need to act on this.",
        "request-dismissal",
        HouseholdWorkStatus::Dismissed,
    );
}

#[test]
fn scanned_image_fixture_uses_the_same_household_administration_contract() {
    let conversations = MemoryConversationPort::default();
    let work = MemoryHouseholdWorkPort::default();
    let source = fixture_source(
        "document-41",
        "image/png",
        "OCR: Sydney Water. Rental property. $96.40 due 20 August 2026.",
    );
    let captured = Arc::new(Mutex::new(None));
    let reasoning = FixtureReasoningPort {
        request: captured.clone(),
        result: Ok(provider_result(
            "request-scanned-image",
            "I found a Sydney Water bill for the rental property, due 20 August.",
            HouseholdWorkProposal {
                operation: HouseholdWorkOperation::Create,
                work_id: None,
                kind: Some(HouseholdWorkKind::Bill),
                summary: Some("Pay the Sydney Water bill".to_owned()),
                status: Some(HouseholdWorkStatus::Active),
                facts: vec![
                    WorkFact {
                        key: WorkFactKey::Provider,
                        value: "Sydney Water".to_owned(),
                        evidence_refs: vec!["document-41".to_owned()],
                        certainty: WorkFactCertainty::Confirmed,
                    },
                    WorkFact {
                        key: WorkFactKey::Amount,
                        value: "$96.40".to_owned(),
                        evidence_refs: vec!["document-41".to_owned()],
                        certainty: WorkFactCertainty::Confirmed,
                    },
                ],
                due_at: Some("2026-08-20".to_owned()),
                urgency: None,
            },
        )),
    };
    let clock = FixedHouseholdAdministrationClock::new("2026-08-05T10:05:00Z");
    let engine =
        HouseholdAdministrationEngine::new(&conversations, &work, &source, &reasoning, &clock);

    let outcome = engine
        .handle_turn(turn_input("Take care of this.", "request-scanned-image"))
        .expect("handle scanned image source");

    assert_eq!(outcome.work.expect("created work").facts.len(), 2);
    let request = captured
        .lock()
        .expect("request lock")
        .clone()
        .expect("captured request");
    assert_eq!(
        request.response_schema_version,
        "household-administration.v1"
    );
    assert_eq!(
        request.source.expect("image source").media_type,
        "image/png"
    );
}

#[test]
fn explicit_new_source_delegation_rejects_a_provider_no_op() {
    let conversations = MemoryConversationPort::default();
    let work = MemoryHouseholdWorkPort::default();
    let source = fixture_source(
        "document-41",
        "image/png",
        "OCR: Sydney Water. Rental property. $96.40 due 20 August 2026.",
    );
    let reasoning = FixtureReasoningPort {
        request: Arc::new(Mutex::new(None)),
        result: Ok(provider_result(
            "request-delegation-no-op",
            "I found the bill. Which property is it for?",
            no_work_proposal(),
        )),
    };
    let clock = FixedHouseholdAdministrationClock::new("2026-08-05T10:05:30Z");
    let engine =
        HouseholdAdministrationEngine::new(&conversations, &work, &source, &reasoning, &clock);

    let failure = engine
        .handle_turn(turn_input("Take care of this.", "request-delegation-no-op"))
        .expect_err("reject a no-op for an explicit new-source delegation");

    assert_eq!(
        failure.category,
        HouseholdAdministrationFailureCategory::MalformedProviderResult
    );
    assert!(work.works.lock().expect("work lock").is_empty());
}

#[test]
fn malformed_provider_result_has_an_exact_failure_category() {
    let conversations = MemoryConversationPort::default();
    let work = MemoryHouseholdWorkPort::default();
    let source = fixture_source("document-41", "application/pdf", "AGL bill");
    let reasoning = FixtureReasoningPort {
        request: Arc::new(Mutex::new(None)),
        result: Ok(provider_result(
            "request-malformed",
            "",
            HouseholdWorkProposal {
                operation: HouseholdWorkOperation::Create,
                work_id: None,
                kind: Some(HouseholdWorkKind::Bill),
                summary: Some("AGL bill".to_owned()),
                status: Some(HouseholdWorkStatus::Active),
                facts: Vec::new(),
                due_at: None,
                urgency: None,
            },
        )),
    };
    let clock = FixedHouseholdAdministrationClock::new("2026-08-05T10:06:00Z");
    let engine =
        HouseholdAdministrationEngine::new(&conversations, &work, &source, &reasoning, &clock);

    let failure = engine
        .handle_turn(turn_input("Take care of this.", "request-malformed"))
        .expect_err("reject malformed provider result");

    assert_eq!(
        failure.category,
        HouseholdAdministrationFailureCategory::MalformedProviderResult
    );
}

#[test]
fn incompatible_contract_version_has_an_exact_failure_category() {
    let conversations = MemoryConversationPort::default();
    let work = MemoryHouseholdWorkPort::default();
    let source = fixture_source("document-41", "application/pdf", "AGL bill");
    let reasoning = FixtureReasoningPort {
        request: Arc::new(Mutex::new(None)),
        result: Err(ReasoningPortError::IncompatibleContractVersion),
    };
    let clock = FixedHouseholdAdministrationClock::new("2026-08-05T10:07:00Z");
    let engine =
        HouseholdAdministrationEngine::new(&conversations, &work, &source, &reasoning, &clock);

    let failure = engine
        .handle_turn(turn_input(
            "Take care of this.",
            "request-incompatible-contract",
        ))
        .expect_err("reject incompatible contract");

    assert_eq!(
        failure.category,
        HouseholdAdministrationFailureCategory::IncompatibleContractVersion
    );
}

#[test]
fn missing_existing_work_has_an_exact_failure_category() {
    let conversations = MemoryConversationPort::default();
    let work = MemoryHouseholdWorkPort::default();
    let source = fixture_source("document-41", "application/pdf", "AGL bill");
    let reasoning = FixtureReasoningPort {
        request: Arc::new(Mutex::new(None)),
        result: Ok(provider_result(
            "request-missing-work",
            "I updated the bill.",
            HouseholdWorkProposal {
                operation: HouseholdWorkOperation::Update,
                work_id: Some("work-that-does-not-exist".to_owned()),
                kind: None,
                summary: None,
                status: Some(HouseholdWorkStatus::Active),
                facts: Vec::new(),
                due_at: None,
                urgency: None,
            },
        )),
    };
    let clock = FixedHouseholdAdministrationClock::new("2026-08-05T10:08:00Z");
    let engine =
        HouseholdAdministrationEngine::new(&conversations, &work, &source, &reasoning, &clock);

    let failure = engine
        .handle_turn(turn_input("Update this.", "request-missing-work"))
        .expect_err("reject missing Household Work");

    assert_eq!(
        failure.category,
        HouseholdAdministrationFailureCategory::MissingExistingWork
    );
}

#[test]
fn invalid_correction_target_has_an_exact_failure_category() {
    let conversations = MemoryConversationPort::default();
    let work = MemoryHouseholdWorkPort::default();
    let source = fixture_source("document-41", "application/pdf", "AGL bill");
    let reasoning = FixtureReasoningPort {
        request: Arc::new(Mutex::new(None)),
        result: Ok(provider_result(
            "request-invalid-correction",
            "I corrected it.",
            no_work_proposal(),
        )),
    };
    let clock = FixedHouseholdAdministrationClock::new("2026-08-05T10:09:00Z");
    let engine =
        HouseholdAdministrationEngine::new(&conversations, &work, &source, &reasoning, &clock);
    let mut input = turn_input(
        "That bill is for the rental property, not our home.",
        "request-invalid-correction",
    );
    input.active_work_reference = Some("missing-work".to_owned());

    let failure = engine
        .handle_turn(input)
        .expect_err("reject correction without a valid target");

    assert_eq!(
        failure.category,
        HouseholdAdministrationFailureCategory::InvalidCorrectionTarget
    );
}

#[test]
fn persistence_failure_has_an_exact_failure_category() {
    let conversations = MemoryConversationPort::default();
    let work = MemoryHouseholdWorkPort {
        works: Mutex::new(Vec::new()),
        fail_save: true,
    };
    let source = fixture_source("document-41", "application/pdf", "AGL bill");
    let reasoning = FixtureReasoningPort {
        request: Arc::new(Mutex::new(None)),
        result: Ok(provider_result(
            "request-persistence-failure",
            "I found the bill.",
            HouseholdWorkProposal {
                operation: HouseholdWorkOperation::Create,
                work_id: None,
                kind: Some(HouseholdWorkKind::Bill),
                summary: Some("Pay the AGL bill".to_owned()),
                status: Some(HouseholdWorkStatus::Active),
                facts: Vec::new(),
                due_at: None,
                urgency: None,
            },
        )),
    };
    let clock = FixedHouseholdAdministrationClock::new("2026-08-05T10:10:00Z");
    let engine =
        HouseholdAdministrationEngine::new(&conversations, &work, &source, &reasoning, &clock);

    let failure = engine
        .handle_turn(turn_input(
            "Take care of this.",
            "request-persistence-failure",
        ))
        .expect_err("surface work and audit persistence failure");

    assert_eq!(
        failure.category,
        HouseholdAdministrationFailureCategory::PersistenceUnavailable
    );
}

fn live_openai_reasoning() -> OpenAiHouseholdAdministrationReasoningAdapter {
    OpenAiHouseholdAdministrationReasoningAdapter::from_env()
        .expect("set server-side OPENAI_API_KEY and explicit LUNA_OPENAI_MODEL")
}

fn sanitised_live_result(
    result: &Arc<Mutex<Option<Result<UntrustedHouseholdAdministrationResult, ReasoningPortError>>>>,
) -> String {
    match result.lock().expect("reasoning result lock").as_ref() {
        Some(Ok(result)) => format!(
            "provider={} model={} operation={:?} work_id={:?} status={:?} facts={:?} clarification={} action_kinds={:?} input_tokens={:?} output_tokens={:?}",
            result.provider_id,
            result.model_id,
            result.work.operation,
            result.work.work_id,
            result.work.status,
            result.work.facts.iter().map(|fact| {
                let value_class = if fact.key == WorkFactKey::Property {
                    let value = fact.value.to_ascii_lowercase();
                    if value.contains("rental") {
                        "rental"
                    } else if value.contains("seabreeze") {
                        "authorised-address"
                    } else if value.contains("home") {
                        "home"
                    } else {
                        "other"
                    }
                } else {
                    "redacted"
                };
                let evidence_classes = fact.evidence_refs.iter().map(|reference| {
                    if reference == "document-41" {
                        "source"
                    } else if reference == "conversation-member" {
                        "conversation"
                    } else if reference == "household-property-seabreeze" {
                        "household-context"
                    } else {
                        "other"
                    }
                }).collect::<Vec<_>>();
                (fact.key.clone(), value_class, evidence_classes)
            }).collect::<Vec<_>>(),
            result.clarification.is_some(),
            result
                .proposed_actions
                .iter()
                .map(|action| action.kind)
                .collect::<Vec<_>>(),
            result.usage.input_tokens,
            result.usage.output_tokens,
        ),
        Some(Err(error)) => format!("reasoning_error={error:?}"),
        None => "reasoning_result=missing".to_owned(),
    }
}

fn live_image_source(png: Vec<u8>) -> FixtureSourcePort {
    FixtureSourcePort {
        source: HouseholdAdministrationSource {
            reference: "document-41".to_owned(),
            filename: "sanitised-household-bill.png".to_owned(),
            media_type: "image/png".to_owned(),
            original_base64: BASE64.encode(&png),
            extracted_text: None,
            original_size_bytes: png.len() as u64,
            extracted_text_truncated: false,
        },
    }
}

#[test]
#[ignore = "requires server-side OPENAI_API_KEY and explicit LUNA_OPENAI_MODEL"]
fn live_openai_reasoning_isolates_clarification_without_desktop_state() {
    let conversations = MemoryConversationPort::default();
    let existing = existing_bill(HouseholdWorkStatus::NeedsClarification);
    let preserved_facts = existing.facts.clone();
    let preserved_actions = existing.proposed_actions.clone();
    let work = MemoryHouseholdWorkPort {
        works: Mutex::new(vec![existing]),
        fail_save: false,
    };
    let source = live_image_source(sanitised_agl_bill_png());
    let captured = Arc::new(Mutex::new(None));
    let reasoning = CapturingReasoningPort {
        inner: live_openai_reasoning(),
        result: captured.clone(),
    };
    let clock = FixedHouseholdAdministrationClock::new("2026-08-05T11:00:00Z");
    let engine =
        HouseholdAdministrationEngine::new(&conversations, &work, &source, &reasoning, &clock);
    let outcome = engine
        .handle_turn(turn_input("The rental property.", "live-clarification"))
        .expect("live clarification result");
    eprintln!("live clarification {}", sanitised_live_result(&captured));
    let updated = outcome.work.expect("updated Household Work");
    assert_eq!(updated.id, "work-1");
    assert!(updated.facts.iter().any(|fact| {
        fact.key == WorkFactKey::Property && fact.value.to_ascii_lowercase().contains("rental")
    }));
    for fact in preserved_facts {
        assert!(updated.facts.contains(&fact));
    }
    assert_eq!(updated.proposed_actions, preserved_actions);
    assert_eq!(work.works.lock().expect("work lock").len(), 1);
    assert!(!outcome.message.trim().is_empty());
    assert!(!outcome.message.trim_start().starts_with('{'));
}

#[test]
#[ignore = "requires server-side OPENAI_API_KEY and explicit LUNA_OPENAI_MODEL"]
fn live_openai_reasoning_isolates_correction_without_desktop_state() {
    let conversations = MemoryConversationPort::default();
    let mut existing = existing_bill(HouseholdWorkStatus::Active);
    existing.facts.push(WorkFact {
        key: WorkFactKey::Property,
        value: "Our home".to_owned(),
        evidence_refs: vec!["document-41".to_owned()],
        certainty: WorkFactCertainty::Likely,
    });
    let preserved_facts = existing
        .facts
        .iter()
        .filter(|fact| fact.key != WorkFactKey::Property)
        .cloned()
        .collect::<Vec<_>>();
    let preserved_actions = existing.proposed_actions.clone();
    let work = MemoryHouseholdWorkPort {
        works: Mutex::new(vec![existing]),
        fail_save: false,
    };
    let source = live_image_source(sanitised_agl_bill_png());
    let captured = Arc::new(Mutex::new(None));
    let reasoning = CapturingReasoningPort {
        inner: live_openai_reasoning(),
        result: captured.clone(),
    };
    let clock = FixedHouseholdAdministrationClock::new("2026-08-05T11:01:00Z");
    let engine =
        HouseholdAdministrationEngine::new(&conversations, &work, &source, &reasoning, &clock);
    let outcome = engine.handle_turn(turn_input(
        "That bill is for the rental property, not our home.",
        "live-correction",
    ));
    eprintln!("live correction {}", sanitised_live_result(&captured));
    let outcome = outcome.expect("live correction result");
    let updated = outcome.work.expect("updated Household Work");
    assert_eq!(updated.id, "work-1");
    assert!(updated.facts.iter().any(|fact| {
        fact.key == WorkFactKey::Property && fact.value.to_ascii_lowercase().contains("rental")
    }));
    assert!(!updated.facts.iter().any(|fact| {
        fact.key == WorkFactKey::Property && fact.value.eq_ignore_ascii_case("our home")
    }));
    for fact in preserved_facts {
        assert!(updated.facts.contains(&fact));
    }
    assert_eq!(updated.proposed_actions, preserved_actions);
    assert_eq!(work.works.lock().expect("work lock").len(), 1);
    assert!(!outcome.message.trim().is_empty());
    assert!(!outcome.message.trim_start().starts_with('{'));
}

#[test]
#[ignore = "requires server-side OPENAI_API_KEY and explicit LUNA_OPENAI_MODEL"]
fn live_openai_reasoning_isolates_scanned_image_without_desktop_state() {
    let conversations = MemoryConversationPort::default();
    let work = MemoryHouseholdWorkPort::default();
    let source = live_image_source(sanitised_sydney_water_bill_png());
    let captured = Arc::new(Mutex::new(None));
    let reasoning = CapturingReasoningPort {
        inner: live_openai_reasoning(),
        result: captured.clone(),
    };
    let clock = FixedHouseholdAdministrationClock::new("2026-08-05T11:02:00Z");
    let engine =
        HouseholdAdministrationEngine::new(&conversations, &work, &source, &reasoning, &clock);
    let outcome = engine
        .handle_turn(turn_input("Take care of this.", "live-scanned-image"))
        .expect("live scanned-image result");
    eprintln!("live scanned image {}", sanitised_live_result(&captured));
    let created = outcome.work.expect("created Household Work");
    assert!(created
        .facts
        .iter()
        .any(|fact| fact.key == WorkFactKey::Provider && fact.value.contains("Sydney Water")));
    assert!(created
        .facts
        .iter()
        .any(|fact| fact.key == WorkFactKey::Amount && fact.value.contains("96.40")));
    assert!(created.facts.iter().any(|fact| {
        fact.key == WorkFactKey::DueDate
            && (fact.value.contains("2026-08-20") || fact.value.contains("20 August 2026"))
    }));
    assert_eq!(work.works.lock().expect("work lock").len(), 1);
    assert!(!outcome.message.trim().is_empty());
}

fn sanitised_agl_bill_png() -> Vec<u8> {
    sanitised_bill_png(&[
        "AGL ENERGY",
        "ACCOUNT 7788",
        "AMOUNT $184.72",
        "DUE 15 AUGUST 2026",
    ])
}

fn sanitised_sydney_water_bill_png() -> Vec<u8> {
    sanitised_bill_png(&[
        "SYDNEY WATER",
        "ACCOUNT 7788",
        "AMOUNT $96.40",
        "DUE 20 AUGUST 2026",
        "RENTAL PROPERTY",
    ])
}

fn sanitised_bill_png(lines: &[&str]) -> Vec<u8> {
    let mut image = ImageBuffer::from_pixel(1_000, 620, Rgb([255_u8, 255_u8, 255_u8]));
    for (index, line) in lines.iter().enumerate() {
        draw_fixture_text(
            &mut image,
            70,
            70 + index as u32 * 100,
            if index == 0 { 10 } else { 7 },
            line,
        );
    }
    let mut png = Cursor::new(Vec::new());
    DynamicImage::ImageRgb8(image)
        .write_to(&mut png, ImageFormat::Png)
        .expect("encode sanitised PNG fixture");
    png.into_inner()
}

fn draw_fixture_text(
    image: &mut ImageBuffer<Rgb<u8>, Vec<u8>>,
    start_x: u32,
    start_y: u32,
    scale: u32,
    text: &str,
) {
    let mut x = start_x;
    for character in text.chars() {
        for (row, bits) in fixture_glyph(character).into_iter().enumerate() {
            for column in 0..5 {
                if bits & (1 << (4 - column)) != 0 {
                    for dx in 0..scale {
                        for dy in 0..scale {
                            image.put_pixel(
                                x + column * scale + dx,
                                start_y + row as u32 * scale + dy,
                                Rgb([20, 35, 55]),
                            );
                        }
                    }
                }
            }
        }
        x += 6 * scale;
    }
}

fn fixture_glyph(character: char) -> [u8; 7] {
    match character {
        'A' => [
            0b01110, 0b10001, 0b10001, 0b11111, 0b10001, 0b10001, 0b10001,
        ],
        'C' => [
            0b01111, 0b10000, 0b10000, 0b10000, 0b10000, 0b10000, 0b01111,
        ],
        'D' => [
            0b11110, 0b10001, 0b10001, 0b10001, 0b10001, 0b10001, 0b11110,
        ],
        'E' => [
            0b11111, 0b10000, 0b10000, 0b11110, 0b10000, 0b10000, 0b11111,
        ],
        'G' => [
            0b01111, 0b10000, 0b10000, 0b10111, 0b10001, 0b10001, 0b01111,
        ],
        'L' => [
            0b10000, 0b10000, 0b10000, 0b10000, 0b10000, 0b10000, 0b11111,
        ],
        'M' => [
            0b10001, 0b11011, 0b10101, 0b10101, 0b10001, 0b10001, 0b10001,
        ],
        'N' => [
            0b10001, 0b11001, 0b10101, 0b10011, 0b10001, 0b10001, 0b10001,
        ],
        'O' => [
            0b01110, 0b10001, 0b10001, 0b10001, 0b10001, 0b10001, 0b01110,
        ],
        'P' => [
            0b11110, 0b10001, 0b10001, 0b11110, 0b10000, 0b10000, 0b10000,
        ],
        'R' => [
            0b11110, 0b10001, 0b10001, 0b11110, 0b10100, 0b10010, 0b10001,
        ],
        'S' => [
            0b01111, 0b10000, 0b10000, 0b01110, 0b00001, 0b00001, 0b11110,
        ],
        'T' => [
            0b11111, 0b00100, 0b00100, 0b00100, 0b00100, 0b00100, 0b00100,
        ],
        'U' => [
            0b10001, 0b10001, 0b10001, 0b10001, 0b10001, 0b10001, 0b01110,
        ],
        'W' => [
            0b10001, 0b10001, 0b10001, 0b10101, 0b10101, 0b11011, 0b10001,
        ],
        'Y' => [
            0b10001, 0b10001, 0b01010, 0b00100, 0b00100, 0b00100, 0b00100,
        ],
        '0' => [
            0b01110, 0b10011, 0b10101, 0b10101, 0b10101, 0b11001, 0b01110,
        ],
        '1' => [
            0b00100, 0b01100, 0b00100, 0b00100, 0b00100, 0b00100, 0b01110,
        ],
        '2' => [
            0b01110, 0b10001, 0b00001, 0b00010, 0b00100, 0b01000, 0b11111,
        ],
        '4' => [
            0b00010, 0b00110, 0b01010, 0b10010, 0b11111, 0b00010, 0b00010,
        ],
        '5' => [
            0b11111, 0b10000, 0b10000, 0b11110, 0b00001, 0b00001, 0b11110,
        ],
        '6' => [
            0b00110, 0b01000, 0b10000, 0b11110, 0b10001, 0b10001, 0b01110,
        ],
        '7' => [
            0b11111, 0b00001, 0b00010, 0b00100, 0b01000, 0b01000, 0b01000,
        ],
        '8' => [
            0b01110, 0b10001, 0b10001, 0b01110, 0b10001, 0b10001, 0b01110,
        ],
        '9' => [
            0b01110, 0b10001, 0b10001, 0b01111, 0b00001, 0b00010, 0b01100,
        ],
        '$' => [
            0b00100, 0b01111, 0b10100, 0b01110, 0b00101, 0b11110, 0b00100,
        ],
        '.' => [0, 0, 0, 0, 0, 0b00110, 0b00110],
        ' ' => [0; 7],
        _ => [0b11111, 0b10001, 0b00010, 0b00100, 0b01000, 0, 0b01000],
    }
}
