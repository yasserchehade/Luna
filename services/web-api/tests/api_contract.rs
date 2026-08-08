use axum::{
    body::Body,
    http::{Request, StatusCode},
};
use http_body_util::BodyExt;
use std::sync::Arc;

use luna_core::{
    HouseholdActionProposal, HouseholdAdministrationReasoning, HouseholdAdministrationRequest,
    HouseholdWorkKind, HouseholdWorkOperation, HouseholdWorkProposal, IntelligenceUsage,
    ProposedActionKind, ReasoningPortError, UntrustedHouseholdAdministrationResult, WorkFact,
    WorkFactCertainty, WorkFactKey,
};
use luna_web_api::{app_with_reasoning, WebConfig};
use tower::ServiceExt;

fn config(data_dir: &tempfile::TempDir) -> WebConfig {
    WebConfig {
        data_dir: data_dir.path().to_path_buf(),
        household_id: "server-household".to_owned(),
        member_id: "server-member".to_owned(),
        member_display_name: "Yasser Chehade".to_owned(),
        household_name: "Chehade household".to_owned(),
    }
}

struct CreateBillReasoning;

struct FailingReasoning;

impl HouseholdAdministrationReasoning for FailingReasoning {
    fn reason(
        &self,
        _request: &HouseholdAdministrationRequest,
    ) -> Result<UntrustedHouseholdAdministrationResult, ReasoningPortError> {
        Err(ReasoningPortError::Unavailable)
    }
}

impl HouseholdAdministrationReasoning for CreateBillReasoning {
    fn reason(
        &self,
        request: &HouseholdAdministrationRequest,
    ) -> Result<UntrustedHouseholdAdministrationResult, ReasoningPortError> {
        let source = request.source.as_ref();
        let normalized = request.current_message.to_ascii_lowercase();
        let existing = request.active_household_work.first();
        let (reply, work) = if normalized.contains("account number") {
            (
                "The account number is NS-123456.".to_owned(),
                HouseholdWorkProposal {
                    operation: HouseholdWorkOperation::None,
                    work_id: None,
                    kind: None,
                    summary: None,
                    status: None,
                    facts: Vec::new(),
                    due_at: None,
                    urgency: None,
                },
            )
        } else if normalized.contains("rental property") {
            (
                "I updated the property and kept the other bill details unchanged.".to_owned(),
                HouseholdWorkProposal {
                    operation: HouseholdWorkOperation::Update,
                    work_id: existing.map(|work| work.id.clone()),
                    kind: None,
                    summary: None,
                    status: None,
                    facts: vec![WorkFact {
                        key: WorkFactKey::Property,
                        value: "Rental property".to_owned(),
                        evidence_refs: vec!["conversation-member".to_owned()],
                        certainty: WorkFactCertainty::Confirmed,
                    }],
                    due_at: None,
                    urgency: None,
                },
            )
        } else if normalized.contains("already paid") || normalized.contains("is complete") {
            (
                "I marked the electricity bill complete and moved it out of today's attention."
                    .to_owned(),
                HouseholdWorkProposal {
                    operation: HouseholdWorkOperation::Update,
                    work_id: existing.map(|work| work.id.clone()),
                    kind: None,
                    summary: None,
                    status: Some(luna_core::HouseholdWorkStatus::Completed),
                    facts: Vec::new(),
                    due_at: None,
                    urgency: None,
                },
            )
        } else {
            let source = source.expect("uploaded source reaches engine");
            (
                "I found an electricity bill for the rental property, due 15 August.".to_owned(),
                HouseholdWorkProposal {
                    operation: HouseholdWorkOperation::Create,
                    work_id: None,
                    kind: Some(HouseholdWorkKind::Bill),
                    summary: Some("Electricity bill needs attention".to_owned()),
                    status: None,
                    facts: [
                        (WorkFactKey::Provider, "Northstar Electricity"),
                        (WorkFactKey::Amount, "$184.72"),
                        (WorkFactKey::DueDate, "15 August 2026"),
                        (WorkFactKey::Property, "Home"),
                        (WorkFactKey::Account, "NS-123456"),
                    ]
                    .into_iter()
                    .map(|(key, value)| WorkFact {
                        key,
                        value: value.to_owned(),
                        evidence_refs: vec![source.reference.clone()],
                        certainty: WorkFactCertainty::Confirmed,
                    })
                    .collect(),
                    due_at: Some("2026-08-15".to_owned()),
                    urgency: Some("normal".to_owned()),
                },
            )
        };
        Ok(UntrustedHouseholdAdministrationResult {
            request_id: request.request_id.clone(),
            provider_id: "openai".to_owned(),
            model_id: "fixture-model".to_owned(),
            reply,
            work,
            clarification: None,
            proposed_actions: if request.source.is_some() {
                vec![HouseholdActionProposal {
                    kind: ProposedActionKind::Reminder,
                    summary: "Remind the household before the bill is due".to_owned(),
                    arguments: std::collections::BTreeMap::from([
                        ("remindAt".to_owned(), "2026-08-12".to_owned()),
                        (
                            "message".to_owned(),
                            "Electricity bill is due soon".to_owned(),
                        ),
                    ]),
                    approval_required: true,
                }]
            } else {
                Vec::new()
            },
            usage: IntelligenceUsage::default(),
        })
    }
}

#[tokio::test]
async fn today_returns_the_server_derived_household_projection() {
    let data_dir = tempfile::tempdir().unwrap();
    let response = app_with_reasoning(config(&data_dir), Arc::new(CreateBillReasoning))
        .unwrap()
        .oneshot(
            Request::builder()
                .uri("/api/today")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let body = response.into_body().collect().await.unwrap().to_bytes();
    let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(json["member"]["displayName"], "Yasser Chehade");
    assert_eq!(json["member"]["householdName"], "Chehade household");
    assert_eq!(json["conversation"], serde_json::json!([]));
    assert_eq!(json["work"], serde_json::json!([]));
}

#[tokio::test]
async fn pdf_upload_returns_a_safe_source_reference_and_persists_metadata() {
    let data_dir = tempfile::tempdir().unwrap();
    let router = app_with_reasoning(config(&data_dir), Arc::new(CreateBillReasoning)).unwrap();
    let boundary = "luna-upload-boundary";
    let body = format!(
        "--{boundary}\r\nContent-Disposition: form-data; name=\"source\"; filename=\"electricity bill.pdf\"\r\nContent-Type: application/pdf\r\n\r\n%PDF-1.4 sanitised fixture\r\n--{boundary}--\r\n"
    );
    let response = router
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/sources")
                .header(
                    "content-type",
                    format!("multipart/form-data; boundary={boundary}"),
                )
                .body(Body::from(body))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::CREATED);
    let bytes = response.into_body().collect().await.unwrap().to_bytes();
    let source: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    assert!(source["sourceId"].as_str().unwrap().starts_with("source-"));
    assert_eq!(source["displayName"], "electricity bill.pdf");
    assert!(source.get("path").is_none());

    let today = router
        .oneshot(
            Request::builder()
                .uri("/api/today")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let bytes = today.into_body().collect().await.unwrap().to_bytes();
    let today: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(today["reviewed"]["documents"], 1);
}

#[tokio::test]
async fn upload_rejects_unsupported_and_oversized_sources() {
    let data_dir = tempfile::tempdir().unwrap();
    let router = app_with_reasoning(config(&data_dir), Arc::new(CreateBillReasoning)).unwrap();

    let unsupported = "--x\r\nContent-Disposition: form-data; name=\"source\"; filename=\"bill.txt\"\r\nContent-Type: text/plain\r\n\r\nbill\r\n--x--\r\n";
    let response = router
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/sources")
                .header("content-type", "multipart/form-data; boundary=x")
                .body(Body::from(unsupported))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::UNSUPPORTED_MEDIA_TYPE);

    let oversized = vec![b'x'; luna_core::MAX_MVP_DOCUMENT_BYTES as usize + 1];
    let mut body = b"--y\r\nContent-Disposition: form-data; name=\"source\"; filename=\"bill.jpg\"\r\nContent-Type: image/jpeg\r\n\r\n".to_vec();
    body.extend(oversized);
    body.extend(b"\r\n--y--\r\n");
    let response = router
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/sources")
                .header("content-type", "multipart/form-data; boundary=y")
                .body(Body::from(body))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::PAYLOAD_TOO_LARGE);
}

#[tokio::test]
async fn failed_reasoning_rolls_back_the_member_message() {
    let data_dir = tempfile::tempdir().unwrap();
    let router = app_with_reasoning(config(&data_dir), Arc::new(FailingReasoning)).unwrap();
    let (status, _) = json_request(
        router.clone(),
        "POST",
        "/api/conversation",
        serde_json::json!({
            "message": "What needs attention?"
        }),
    )
    .await;
    assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE);

    let today = router
        .oneshot(
            Request::builder()
                .uri("/api/today")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let today = today.into_body().collect().await.unwrap().to_bytes();
    let today: serde_json::Value = serde_json::from_slice(&today).unwrap();
    assert_eq!(today["conversation"], serde_json::json!([]));
    assert_eq!(today["work"], serde_json::json!([]));
}

#[tokio::test]
async fn uploaded_document_turn_survives_backend_restart() {
    let data_dir = tempfile::tempdir().unwrap();
    let router = app_with_reasoning(config(&data_dir), Arc::new(CreateBillReasoning)).unwrap();
    let boundary = "luna-e2e-boundary";
    let upload_body = format!(
        "--{boundary}\r\nContent-Disposition: form-data; name=\"source\"; filename=\"electricity.pdf\"\r\nContent-Type: application/pdf\r\n\r\n%PDF-1.4 sanitised electricity bill\r\n--{boundary}--\r\n"
    );
    let upload = router
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/sources")
                .header(
                    "content-type",
                    format!("multipart/form-data; boundary={boundary}"),
                )
                .body(Body::from(upload_body))
                .unwrap(),
        )
        .await
        .unwrap();
    let upload = upload.into_body().collect().await.unwrap().to_bytes();
    let upload: serde_json::Value = serde_json::from_slice(&upload).unwrap();

    let turn = router
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/conversation")
                .header("content-type", "application/json")
                .body(Body::from(
                    serde_json::json!({
                        "message": "Take care of this.",
                        "sourceId": upload["sourceId"],
                        "contextualWorkIds": []
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(turn.status(), StatusCode::OK);
    let turn = turn.into_body().collect().await.unwrap().to_bytes();
    let turn: serde_json::Value = serde_json::from_slice(&turn).unwrap();
    assert_eq!(turn["affectedWorkIds"].as_array().unwrap().len(), 1);
    assert_eq!(turn["briefing"]["work"].as_array().unwrap().len(), 1);
    assert_eq!(
        turn["lunaMessage"]["body"],
        "I found an electricity bill for the rental property, due 15 August."
    );
    let work_id = turn["affectedWorkIds"][0].as_str().unwrap().to_owned();

    let restarted = app_with_reasoning(config(&data_dir), Arc::new(CreateBillReasoning)).unwrap();
    let today = restarted
        .oneshot(
            Request::builder()
                .uri("/api/today")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let today = today.into_body().collect().await.unwrap().to_bytes();
    let today: serde_json::Value = serde_json::from_slice(&today).unwrap();
    assert_eq!(today["conversation"].as_array().unwrap().len(), 2);
    assert_eq!(today["work"][0]["id"], work_id);
    assert_eq!(
        today["work"][0]["facts"][0]["value"],
        "Northstar Electricity"
    );
}

async fn json_request(
    router: axum::Router,
    method: &str,
    uri: &str,
    value: serde_json::Value,
) -> (StatusCode, serde_json::Value) {
    let response = router
        .oneshot(
            Request::builder()
                .method(method)
                .uri(uri)
                .header("content-type", "application/json")
                .body(Body::from(value.to_string()))
                .unwrap(),
        )
        .await
        .unwrap();
    let status = response.status();
    let body = response.into_body().collect().await.unwrap().to_bytes();
    (status, serde_json::from_slice(&body).unwrap())
}

#[tokio::test]
async fn founder_uploaded_document_journey_is_read_only_correctable_completable_and_durable() {
    let data_dir = tempfile::tempdir().unwrap();
    let router = app_with_reasoning(config(&data_dir), Arc::new(CreateBillReasoning)).unwrap();
    let boundary = "luna-founder-boundary";
    let upload_body = format!(
        "--{boundary}\r\nContent-Disposition: form-data; name=\"source\"; filename=\"electricity.png\"\r\nContent-Type: image/png\r\n\r\nPNG sanitised scanned bill\r\n--{boundary}--\r\n"
    );
    let upload = router
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/sources")
                .header(
                    "content-type",
                    format!("multipart/form-data; boundary={boundary}"),
                )
                .body(Body::from(upload_body))
                .unwrap(),
        )
        .await
        .unwrap();
    let upload = upload.into_body().collect().await.unwrap().to_bytes();
    let upload: serde_json::Value = serde_json::from_slice(&upload).unwrap();

    let (status, created) = json_request(
        router.clone(),
        "POST",
        "/api/conversation",
        serde_json::json!({
            "message": "Take care of this.", "sourceId": upload["sourceId"]
        }),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let work_id = created["affectedWorkIds"][0].as_str().unwrap().to_owned();
    assert_eq!(created["briefing"]["work"].as_array().unwrap().len(), 1);

    let (_, answer) = json_request(
        router.clone(),
        "POST",
        "/api/conversation",
        serde_json::json!({
            "message": "What is the account number?"
        }),
    )
    .await;
    assert_eq!(
        answer["lunaMessage"]["body"],
        "The account number is NS-123456."
    );
    assert_eq!(answer["affectedWorkIds"], serde_json::json!([]));
    assert_eq!(
        answer["briefing"]["work"][0]["facts"]
            .as_array()
            .unwrap()
            .len(),
        5
    );

    let (_, corrected) = json_request(
        router.clone(),
        "POST",
        "/api/conversation",
        serde_json::json!({
            "message": "That's for the rental property."
        }),
    )
    .await;
    assert_eq!(
        corrected["affectedWorkIds"],
        serde_json::json!([work_id.clone()])
    );
    assert_eq!(corrected["briefing"]["work"].as_array().unwrap().len(), 1);
    let facts = corrected["briefing"]["work"][0]["facts"]
        .as_array()
        .unwrap();
    assert_eq!(
        facts.iter().find(|fact| fact["key"] == "property").unwrap()["value"],
        "Rental property"
    );
    assert_eq!(
        facts.iter().find(|fact| fact["key"] == "account").unwrap()["value"],
        "NS-123456"
    );

    let (_, completed) = json_request(
        router.clone(),
        "POST",
        "/api/conversation",
        serde_json::json!({
            "message": "I already paid the electricity bill."
        }),
    )
    .await;
    assert_eq!(
        completed["affectedWorkIds"],
        serde_json::json!([work_id.clone()])
    );
    assert_eq!(completed["briefing"]["work"], serde_json::json!([]));

    let restarted = app_with_reasoning(config(&data_dir), Arc::new(CreateBillReasoning)).unwrap();
    let today = restarted
        .clone()
        .oneshot(
            Request::builder()
                .uri("/api/today")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let today = today.into_body().collect().await.unwrap().to_bytes();
    let today: serde_json::Value = serde_json::from_slice(&today).unwrap();
    assert_eq!(today["work"], serde_json::json!([]));
    assert_eq!(today["conversation"].as_array().unwrap().len(), 8);

    let detail = restarted
        .oneshot(
            Request::builder()
                .uri(format!("/api/household-work/{work_id}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let detail = detail.into_body().collect().await.unwrap().to_bytes();
    let detail: serde_json::Value = serde_json::from_slice(&detail).unwrap();
    assert_eq!(detail["status"], "completed");
    assert!(detail["auditHistory"].as_array().unwrap().len() >= 3);
}

#[tokio::test]
async fn explicit_approval_is_validated_and_persisted_by_luna_without_openai_authority() {
    let data_dir = tempfile::tempdir().unwrap();
    let router = app_with_reasoning(config(&data_dir), Arc::new(CreateBillReasoning)).unwrap();
    let boundary = "luna-approve-boundary";
    let upload_body = format!(
        "--{boundary}\r\nContent-Disposition: form-data; name=\"source\"; filename=\"bill.pdf\"\r\nContent-Type: application/pdf\r\n\r\n%PDF fixture\r\n--{boundary}--\r\n"
    );
    let upload = router
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/sources")
                .header(
                    "content-type",
                    format!("multipart/form-data; boundary={boundary}"),
                )
                .body(Body::from(upload_body))
                .unwrap(),
        )
        .await
        .unwrap()
        .into_body()
        .collect()
        .await
        .unwrap()
        .to_bytes();
    let upload: serde_json::Value = serde_json::from_slice(&upload).unwrap();
    let (_, created) = json_request(
        router.clone(),
        "POST",
        "/api/conversation",
        serde_json::json!({
            "message": "Take care of this.", "sourceId": upload["sourceId"]
        }),
    )
    .await;
    let work = &created["briefing"]["work"][0];
    let work_id = work["id"].as_str().unwrap();
    let action_id = work["proposedAction"]["id"].as_str().unwrap();

    let response = router
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/api/household-work/{work_id}/approve/{action_id}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let response = response.into_body().collect().await.unwrap().to_bytes();
    let response: serde_json::Value = serde_json::from_slice(&response).unwrap();
    assert_eq!(response["work"]["status"], "upcoming");
    assert!(response["work"].get("proposedAction").is_none());
    assert!(response["work"]["auditHistory"]
        .as_array()
        .unwrap()
        .iter()
        .any(|event| event.as_str().unwrap().contains("approved")));
}

#[tokio::test]
async fn explicit_fact_completion_and_dismissal_endpoints_use_luna_owned_commands() {
    let data_dir = tempfile::tempdir().unwrap();
    let router = app_with_reasoning(config(&data_dir), Arc::new(CreateBillReasoning)).unwrap();
    let first = upload_and_create(&router, "first").await;
    let first_id = first["affectedWorkIds"][0].as_str().unwrap();

    let (status, corrected) = json_request(
        router.clone(),
        "POST",
        &format!("/api/household-work/{first_id}/facts"),
        serde_json::json!({ "factKey": "property", "value": "Rental property" }),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(corrected["work"]["householdEntity"], "Rental property");
    assert!(corrected["work"]["facts"]
        .as_array()
        .unwrap()
        .iter()
        .any(|fact| { fact["key"] == "account" && fact["value"] == "NS-123456" }));

    let response = router
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/api/household-work/{first_id}/complete"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let completed: serde_json::Value =
        serde_json::from_slice(&response.into_body().collect().await.unwrap().to_bytes()).unwrap();
    assert_eq!(completed["work"]["status"], "completed");
    assert_eq!(completed["briefing"]["work"], serde_json::json!([]));

    let second = upload_and_create(&router, "second").await;
    let second_id = second["affectedWorkIds"][0].as_str().unwrap();
    let response = router
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/api/household-work/{second_id}/dismiss"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let dismissed: serde_json::Value =
        serde_json::from_slice(&response.into_body().collect().await.unwrap().to_bytes()).unwrap();
    assert_eq!(dismissed["work"]["status"], "dismissed");
    assert_eq!(dismissed["briefing"]["work"], serde_json::json!([]));
}

async fn upload_and_create(router: &axum::Router, suffix: &str) -> serde_json::Value {
    let boundary = format!("luna-command-{suffix}");
    let upload_body = format!(
        "--{boundary}\r\nContent-Disposition: form-data; name=\"source\"; filename=\"bill-{suffix}.pdf\"\r\nContent-Type: application/pdf\r\n\r\n%PDF fixture {suffix}\r\n--{boundary}--\r\n"
    );
    let upload = router
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/sources")
                .header(
                    "content-type",
                    format!("multipart/form-data; boundary={boundary}"),
                )
                .body(Body::from(upload_body))
                .unwrap(),
        )
        .await
        .unwrap()
        .into_body()
        .collect()
        .await
        .unwrap()
        .to_bytes();
    let upload: serde_json::Value = serde_json::from_slice(&upload).unwrap();
    json_request(
        router.clone(),
        "POST",
        "/api/conversation",
        serde_json::json!({
            "message": "Take care of this.", "sourceId": upload["sourceId"]
        }),
    )
    .await
    .1
}
