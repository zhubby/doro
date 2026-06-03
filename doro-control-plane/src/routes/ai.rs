use crate::agent_streams::agent_task_error_message;
use crate::auth::CurrentUser;
use crate::error::{AppError, ai_model_provider_store_app_error, normalize_optional_text};
use crate::prelude::*;
use crate::routes::scheduled_tasks::required_text;
use crate::routes::tasks::ensure_agent_run_ready;
use crate::state::AppState;
use url::Url;

pub(crate) async fn list_ai_model_providers(
    State(state): State<AppState>,
) -> Result<Json<ListAiModelProvidersResponse>, AppError> {
    Ok(Json(ListAiModelProvidersResponse {
        items: state.store.ai_model_providers().list().await?,
    }))
}

pub(crate) async fn get_ai_model_provider(
    State(state): State<AppState>,
    AxumPath(provider_id): AxumPath<Uuid>,
) -> Result<Json<AiModelProviderResponse>, AppError> {
    let item = state
        .store
        .ai_model_providers()
        .get(provider_id)
        .await?
        .ok_or_else(|| AppError::status(StatusCode::NOT_FOUND, "ai provider not found"))?;
    Ok(Json(AiModelProviderResponse { item }))
}

pub(crate) async fn create_ai_model_provider(
    State(state): State<AppState>,
    Json(request): Json<CreateAiModelProviderRequest>,
) -> Result<Json<AiModelProviderResponse>, AppError> {
    let now = Utc::now();
    let item = state
        .store
        .ai_model_providers()
        .create(NewAiModelProvider {
            id: Uuid::new_v4(),
            name: required_text(request.name, "ai provider name is required")?,
            base_url: validate_provider_base_url(request.base_url)?,
            default_model: required_text(
                request.default_model,
                "ai provider default_model is required",
            )?,
            timeout_seconds: request.timeout_seconds,
            api_key_secret: required_text(request.api_key, "ai provider api_key is required")?,
            enabled: request.enabled,
            created_at: now,
        })
        .await
        .map_err(ai_model_provider_store_app_error)?;

    Ok(Json(AiModelProviderResponse { item }))
}

pub(crate) async fn update_ai_model_provider(
    State(state): State<AppState>,
    AxumPath(provider_id): AxumPath<Uuid>,
    Json(request): Json<UpdateAiModelProviderRequest>,
) -> Result<Json<AiModelProviderResponse>, AppError> {
    let item = state
        .store
        .ai_model_providers()
        .update(
            provider_id,
            AiModelProviderChanges {
                name: request.name.map(|name| name.trim().to_string()),
                base_url: request
                    .base_url
                    .map(validate_provider_base_url)
                    .transpose()?,
                default_model: request
                    .default_model
                    .map(|default_model| default_model.trim().to_string()),
                timeout_seconds: request.timeout_seconds,
                api_key_secret: normalize_optional_text(request.api_key),
                enabled: request.enabled,
            },
        )
        .await
        .map_err(ai_model_provider_store_app_error)?
        .ok_or_else(|| AppError::status(StatusCode::NOT_FOUND, "ai provider not found"))?;

    Ok(Json(AiModelProviderResponse { item }))
}

pub(crate) async fn delete_ai_model_provider(
    State(state): State<AppState>,
    AxumPath(provider_id): AxumPath<Uuid>,
) -> Result<StatusCode, AppError> {
    if !state.store.ai_model_providers().delete(provider_id).await? {
        return Err(AppError::status(
            StatusCode::NOT_FOUND,
            "ai provider not found",
        ));
    }
    Ok(StatusCode::NO_CONTENT)
}

pub(crate) async fn list_ai_conversations(
    State(state): State<AppState>,
) -> Result<Json<ListAiConversationsResponse>, AppError> {
    Ok(Json(ListAiConversationsResponse {
        items: state.store.ai_chats().list_conversations().await?,
    }))
}

pub(crate) async fn create_ai_conversation(
    State(state): State<AppState>,
    Extension(current_user): Extension<CurrentUser>,
    Json(request): Json<CreateAiConversationRequest>,
) -> Result<Json<AiConversationResponse>, AppError> {
    let now = Utc::now();
    let title = request
        .title
        .and_then(|title| {
            let title = title.trim().to_string();
            (!title.is_empty()).then_some(title)
        })
        .unwrap_or_else(|| "新 AI 对话".to_string());
    let item = state
        .store
        .ai_chats()
        .create_conversation(NewAiConversation {
            id: Uuid::new_v4(),
            title,
            created_by: current_user.username,
            created_at: now,
        })
        .await?;
    Ok(Json(AiConversationResponse {
        item,
        messages: Vec::new(),
        events: Vec::new(),
    }))
}

pub(crate) async fn get_ai_conversation(
    State(state): State<AppState>,
    AxumPath(conversation_id): AxumPath<Uuid>,
) -> Result<Json<AiConversationResponse>, AppError> {
    conversation_response(&state, conversation_id)
        .await
        .map(Json)
}

pub(crate) async fn delete_ai_conversation(
    State(state): State<AppState>,
    AxumPath(conversation_id): AxumPath<Uuid>,
) -> Result<StatusCode, AppError> {
    if state
        .store
        .ai_chats()
        .delete_conversation(conversation_id)
        .await?
    {
        return Ok(StatusCode::NO_CONTENT);
    }
    Err(AppError::status(
        StatusCode::NOT_FOUND,
        "ai conversation not found",
    ))
}

pub(crate) async fn create_ai_chat_turn(
    State(state): State<AppState>,
    Extension(current_user): Extension<CurrentUser>,
    AxumPath(conversation_id): AxumPath<Uuid>,
    Json(request): Json<CreateAiChatTurnRequest>,
) -> Result<Json<CreateAiChatTurnResponse>, AppError> {
    if state
        .store
        .ai_chats()
        .get_conversation(conversation_id)
        .await?
        .is_none()
    {
        return Err(AppError::status(
            StatusCode::NOT_FOUND,
            "ai conversation not found",
        ));
    }
    ensure_agent_run_ready(&state, request.host_id).await?;
    let provider = load_chat_provider(&state, request.ai_provider_id).await?;
    let model = required_text(request.model, "model is required")?;
    let content = required_text(request.content, "chat message content is required")?;
    let now = Utc::now();

    let user_message = state
        .store
        .ai_chats()
        .create_message(NewAiChatMessage {
            id: Uuid::new_v4(),
            conversation_id,
            role: AiChatMessageRole::User,
            status: AiChatMessageStatus::Succeeded,
            content: content.clone(),
            task_id: None,
            host_id: Some(request.host_id),
            ai_provider_id: Some(request.ai_provider_id),
            model: Some(model.clone()),
            metadata: serde_json::json!({}),
            created_at: now,
        })
        .await?;

    let step_id = Uuid::new_v4();
    let task = state
        .store
        .tasks()
        .create_with_steps(NewTask {
            id: Uuid::new_v4(),
            host_id: Some(request.host_id),
            title: chat_task_title(&content),
            prompt: Some(content.clone()),
            status: TaskStatus::Queued,
            created_by: current_user.username,
            created_at: now,
            metadata: serde_json::json!({
                "resource": "ai_chat_turn",
                "conversation_id": conversation_id,
                "user_message_id": user_message.id,
                "ai_provider": chat_provider_metadata(&provider, &model),
            }),
            create_step_approvals: false,
            steps: vec![TaskStep {
                id: step_id,
                capability: CapabilityName::AgentRun,
                risk: CapabilityRisk::Medium,
                summary: "Run AI chat turn on Agent".to_string(),
                status: TaskStepStatus::Pending,
                payload: serde_json::json!({
                    "resource": "ai_chat_turn",
                    "conversation_id": conversation_id,
                    "user_message_id": user_message.id,
                    "ai_provider": chat_provider_metadata(&provider, &model),
                }),
            }],
        })
        .await?;

    let assistant_message = state
        .store
        .ai_chats()
        .create_message(NewAiChatMessage {
            id: Uuid::new_v4(),
            conversation_id,
            role: AiChatMessageRole::Assistant,
            status: AiChatMessageStatus::Pending,
            content: String::new(),
            task_id: Some(task.id),
            host_id: Some(request.host_id),
            ai_provider_id: Some(request.ai_provider_id),
            model: Some(model.clone()),
            metadata: serde_json::json!({
                "user_message_id": user_message.id,
            }),
            created_at: now,
        })
        .await?;

    let messages = state
        .store
        .ai_chats()
        .list_messages(conversation_id)
        .await?;
    let messages_json = chat_messages_for_agent(&messages, assistant_message.id)?;
    let dispatch_state = state.clone();
    let dispatch_provider = provider;
    let dispatch_model = model;
    let dispatch_assistant_message_id = assistant_message.id;
    let dispatch_task_id = task.id;
    tokio::spawn(async move {
        dispatch_ai_chat_turn(
            dispatch_state,
            request.host_id,
            step_id,
            dispatch_task_id,
            conversation_id,
            user_message.id,
            dispatch_assistant_message_id,
            messages_json,
            dispatch_provider,
            dispatch_model,
        )
        .await;
    });

    Ok(Json(CreateAiChatTurnResponse {
        user_message,
        assistant_message,
        task,
    }))
}

#[derive(Debug, Deserialize)]
pub(crate) struct AiChatStreamQuery {
    message_id: Uuid,
    token: String,
}

pub(crate) async fn ai_chat_stream(
    State(state): State<AppState>,
    AxumPath(conversation_id): AxumPath<Uuid>,
    Query(query): Query<AiChatStreamQuery>,
) -> Result<Sse<impl futures_util::Stream<Item = Result<Event, Infallible>>>, AppError> {
    state.auth.verify_access_token(&query.token)?;
    let messages = state
        .store
        .ai_chats()
        .list_messages(conversation_id)
        .await?;
    if !messages
        .iter()
        .any(|message| message.id == query.message_id)
    {
        return Err(AppError::status(
            StatusCode::NOT_FOUND,
            "ai chat message not found",
        ));
    }
    let past_events = state
        .store
        .ai_chats()
        .list_message_events(query.message_id)
        .await?
        .into_iter()
        .map(stream_event_from_chat_event)
        .collect::<VecDeque<_>>();
    let receiver = state.chat_streams.subscribe(query.message_id).await;
    let stream = futures_util::stream::unfold(
        (past_events, receiver),
        |(mut past_events, mut receiver)| async move {
            if let Some(event) = past_events.pop_front() {
                return Some((Ok(sse_chat_event(event)), (past_events, receiver)));
            }
            loop {
                match receiver.recv().await {
                    Ok(event) => return Some((Ok(sse_chat_event(event)), (past_events, receiver))),
                    Err(broadcast::error::RecvError::Lagged(_)) => continue,
                    Err(broadcast::error::RecvError::Closed) => return None,
                }
            }
        },
    );

    Ok(Sse::new(stream).keep_alive(KeepAlive::default()))
}

fn validate_provider_base_url(value: String) -> Result<String, AppError> {
    let value = required_text(value, "ai provider base_url is required")?
        .trim_end_matches('/')
        .to_string();
    let parsed = Url::parse(&value).map_err(|_| {
        AppError::status(StatusCode::BAD_REQUEST, "ai provider base_url is invalid")
    })?;
    match parsed.scheme() {
        "http" | "https" => Ok(value),
        _ => Err(AppError::status(
            StatusCode::BAD_REQUEST,
            "ai provider base_url must use http or https",
        )),
    }
}

async fn conversation_response(
    state: &AppState,
    conversation_id: Uuid,
) -> Result<AiConversationResponse, AppError> {
    let item = state
        .store
        .ai_chats()
        .get_conversation(conversation_id)
        .await?
        .ok_or_else(|| AppError::status(StatusCode::NOT_FOUND, "ai conversation not found"))?;
    let messages = state
        .store
        .ai_chats()
        .list_messages(conversation_id)
        .await?;
    let events = state.store.ai_chats().list_events(conversation_id).await?;
    Ok(AiConversationResponse {
        item,
        messages,
        events,
    })
}

async fn dispatch_ai_chat_turn(
    state: AppState,
    host_id: Uuid,
    step_id: Uuid,
    task_id: Uuid,
    conversation_id: Uuid,
    user_message_id: Uuid,
    assistant_message_id: Uuid,
    messages_json: String,
    provider: StoredAiModelProviderSecret,
    model: String,
) {
    let now = Utc::now();
    let agent_id = match ensure_agent_run_ready(&state, host_id).await {
        Ok(agent_id) => agent_id,
        Err(error) => {
            fail_chat_turn(
                &state,
                conversation_id,
                assistant_message_id,
                task_id,
                error.0.to_string(),
            )
            .await;
            return;
        }
    };
    if let Err(error) = state
        .store
        .tasks()
        .update_status(task_id, TaskStatus::Running, None, None)
        .await
    {
        fail_chat_turn(
            &state,
            conversation_id,
            assistant_message_id,
            task_id,
            error.to_string(),
        )
        .await;
        return;
    }
    let _ = state
        .store
        .tasks()
        .update_step_status(step_id, "running")
        .await;
    let _ = state
        .store
        .ai_chats()
        .update_message(
            assistant_message_id,
            AiChatMessageChanges {
                status: Some(AiChatMessageStatus::Running),
                updated_at: Some(now),
                ..AiChatMessageChanges::default()
            },
        )
        .await;
    let _ = state
        .store
        .tasks()
        .create_run(NewTaskRun {
            id: Uuid::new_v4(),
            task_id,
            step_id: Some(step_id),
            agent_id,
            status: "running".to_string(),
            command_id: None,
            started_at: Some(now),
            finished_at: None,
            result_json: serde_json::json!({}),
            error_message: None,
        })
        .await;

    let result = state
        .agent_streams
        .start_agent_chat_turn(
            host_id,
            grpc::RunAgentChatTurnCommand {
                command_id: String::new(),
                conversation_id: conversation_id.to_string(),
                user_message_id: user_message_id.to_string(),
                assistant_message_id: assistant_message_id.to_string(),
                task_id: task_id.to_string(),
                messages_json,
                ai_provider: Some(grpc_ai_provider_config_with_model(provider, model)),
            },
        )
        .await;

    if let Err(error) = result {
        fail_chat_turn(
            &state,
            conversation_id,
            assistant_message_id,
            task_id,
            agent_task_error_message(error),
        )
        .await;
    }
}

async fn fail_chat_turn(
    state: &AppState,
    conversation_id: Uuid,
    message_id: Uuid,
    task_id: Uuid,
    message: String,
) {
    let now = Utc::now();
    let _ = state
        .store
        .ai_chats()
        .update_message(
            message_id,
            AiChatMessageChanges {
                status: Some(AiChatMessageStatus::Failed),
                updated_at: Some(now),
                ..AiChatMessageChanges::default()
            },
        )
        .await;
    let _ = state
        .store
        .tasks()
        .update_status(
            task_id,
            TaskStatus::Failed,
            Some(now),
            Some(message.clone()),
        )
        .await;
    let event = NewAiChatEvent {
        id: Uuid::new_v4(),
        conversation_id,
        message_id,
        kind: AiChatEventKind::Error,
        content: Some(message.clone()),
        payload: serde_json::json!({ "message": message }),
        created_at: now,
    };
    if let Ok(event) = state.store.ai_chats().record_event(event).await {
        state
            .chat_streams
            .publish(stream_event_from_chat_event(event))
            .await;
    }
}

async fn load_chat_provider(
    state: &AppState,
    provider_id: Uuid,
) -> Result<StoredAiModelProviderSecret, AppError> {
    let provider = state
        .store
        .ai_model_providers()
        .get_secret(provider_id)
        .await?
        .ok_or_else(|| AppError::status(StatusCode::NOT_FOUND, "ai provider not found"))?;
    if !provider.enabled {
        return Err(AppError::status(
            StatusCode::CONFLICT,
            "ai provider is disabled",
        ));
    }
    if provider.api_key_secret.trim().is_empty() {
        return Err(AppError::status(
            StatusCode::CONFLICT,
            "ai provider api_key is not configured",
        ));
    }
    Ok(provider)
}

fn chat_provider_metadata(provider: &StoredAiModelProviderSecret, model: &str) -> Value {
    serde_json::json!({
        "id": provider.id,
        "name": provider.name,
        "provider_type": "openai_responses",
        "base_url": provider.base_url,
        "model": model,
        "timeout_seconds": provider.timeout_seconds,
    })
}

fn grpc_ai_provider_config_with_model(
    provider: StoredAiModelProviderSecret,
    model: String,
) -> grpc::AgentAiProviderConfig {
    grpc::AgentAiProviderConfig {
        provider_type: "openai_responses".to_string(),
        name: provider.name,
        base_url: provider.base_url,
        model,
        api_key: provider.api_key_secret,
        timeout_seconds: provider.timeout_seconds,
    }
}

fn chat_messages_for_agent(
    messages: &[AiChatMessage],
    current_assistant_message_id: Uuid,
) -> Result<String, AppError> {
    let items = messages
        .iter()
        .filter(|message| message.id != current_assistant_message_id)
        .filter(|message| !message.content.trim().is_empty())
        .map(|message| {
            serde_json::json!({
                "role": match message.role {
                    AiChatMessageRole::Assistant => "assistant",
                    AiChatMessageRole::Tool => "tool",
                    AiChatMessageRole::User => "user",
                },
                "content": message.content,
            })
        })
        .collect::<Vec<_>>();
    serde_json::to_string(&items).map_err(AppError::from)
}

fn chat_task_title(content: &str) -> String {
    let title = content.trim().chars().take(32).collect::<String>();
    if title.is_empty() {
        "AI 聊天回合".to_string()
    } else {
        format!("AI 聊天：{title}")
    }
}

fn stream_event_from_chat_event(event: AiChatEvent) -> AiChatStreamEvent {
    AiChatStreamEvent {
        event_id: event.id,
        conversation_id: event.conversation_id,
        message_id: event.message_id,
        kind: event.kind,
        content: event.content,
        payload: event.payload,
        created_at: event.created_at,
    }
}

fn sse_chat_event(event: AiChatStreamEvent) -> Event {
    match serde_json::to_string(&event) {
        Ok(data) => Event::default().event("ai_chat").data(data),
        Err(_) => Event::default().event("ai_chat").data("{}"),
    }
}
