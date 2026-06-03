use crate::error::AppError;
use crate::prelude::*;
use crate::state::AppState;
use jsonwebtoken::DecodingKey;
use jsonwebtoken::EncodingKey;
use jsonwebtoken::Header;
use jsonwebtoken::Validation;

#[derive(Debug, Clone)]
pub struct AuthService {
    jwt_secret: String,
}

#[derive(Debug, Clone)]
pub struct CurrentUser {
    pub id: Uuid,
    pub username: String,
    pub role: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]

pub(crate) struct Claims {
    sub: String,
    username: String,
    role: String,
    iat: i64,
    exp: i64,
    jti: String,
    typ: String,
}

pub(crate) async fn auth_status(
    State(state): State<AppState>,
) -> Result<Json<AuthStatusResponse>, AppError> {
    Ok(Json(AuthStatusResponse {
        registration_open: state.store.users().registration_open().await?,
    }))
}

pub(crate) async fn register(
    State(state): State<AppState>,
    Json(request): Json<RegisterRequest>,
) -> Result<Json<AuthTokenResponse>, AppError> {
    validate_username(&request.username)?;
    validate_password(&request.password)?;
    let now = Utc::now();
    let password_hash = hash_password(&request.password)?;
    let user = state
        .store
        .users()
        .create_first_admin(NewUser {
            id: Uuid::new_v4(),
            username: request.username.trim().to_lowercase(),
            display_name: display_name_or_username(&request.display_name, &request.username),
            password_hash,
            role: "admin".to_string(),
            created_at: now,
        })
        .await
        .map_err(|error| {
            if error.to_string().contains("registration is closed") {
                AppError::status(StatusCode::CONFLICT, "registration is closed")
            } else {
                AppError::from(error)
            }
        })?;
    state.store.users().mark_login(user.id, now).await?;

    Ok(Json(issue_token_pair(&state, user, now).await?))
}

pub(crate) async fn login(
    State(state): State<AppState>,
    Json(request): Json<LoginRequest>,
) -> Result<Json<AuthTokenResponse>, AppError> {
    let username = request.username.trim().to_lowercase();
    let Some(user) = state.store.users().find_by_username(&username).await? else {
        return Err(AppError::status(
            StatusCode::UNAUTHORIZED,
            "invalid credentials",
        ));
    };
    if user.status != "active" || !verify_password(&request.password, &user.password_hash)? {
        return Err(AppError::status(
            StatusCode::UNAUTHORIZED,
            "invalid credentials",
        ));
    }

    let now = Utc::now();
    state.store.users().mark_login(user.id, now).await?;
    Ok(Json(issue_token_pair(&state, user, now).await?))
}

pub(crate) async fn refresh(
    State(state): State<AppState>,
    Json(request): Json<RefreshTokenRequest>,
) -> Result<Json<AuthTokenResponse>, AppError> {
    let now = Utc::now();
    let Some(stored_token) = state
        .store
        .refresh_tokens()
        .find_by_token(&request.refresh_token)
        .await?
    else {
        return Err(AppError::status(
            StatusCode::UNAUTHORIZED,
            "invalid refresh token",
        ));
    };
    if stored_token.status != "active" || stored_token.revoked_at.is_some() {
        state
            .store
            .refresh_tokens()
            .revoke_all_for_user(stored_token.user_id, now)
            .await?;
        return Err(AppError::status(
            StatusCode::UNAUTHORIZED,
            "invalid refresh token",
        ));
    }
    if stored_token.expires_at <= now {
        return Err(AppError::status(
            StatusCode::UNAUTHORIZED,
            "refresh token expired",
        ));
    }
    let Some(user) = state.store.users().find_by_id(stored_token.user_id).await? else {
        return Err(AppError::status(
            StatusCode::UNAUTHORIZED,
            "invalid refresh token",
        ));
    };
    if user.status != "active" {
        return Err(AppError::status(
            StatusCode::UNAUTHORIZED,
            "user is disabled",
        ));
    }

    let refresh_token = generate_refresh_token();
    state
        .store
        .refresh_tokens()
        .rotate(
            stored_token.id,
            NewRefreshToken {
                id: Uuid::new_v4(),
                user_id: user.id,
                token: refresh_token.clone(),
                created_at: now,
                expires_at: now + ChronoDuration::days(30),
            },
            now,
        )
        .await?;
    let (access_token, expires_at) = state.auth.issue_access_token(&user, now)?;
    Ok(Json(AuthTokenResponse {
        access_token,
        refresh_token,
        expires_at,
        user: user_summary(&user),
    }))
}

pub(crate) async fn me(
    State(state): State<AppState>,
    Extension(current_user): Extension<CurrentUser>,
) -> Result<Json<CurrentUserResponse>, AppError> {
    let user = active_current_user(&state, current_user.id).await?;
    Ok(Json(CurrentUserResponse {
        user: user_summary(&user),
    }))
}

pub(crate) async fn update_me(
    State(state): State<AppState>,
    Extension(current_user): Extension<CurrentUser>,
    Json(request): Json<UpdateCurrentUserRequest>,
) -> Result<Json<UpdateCurrentUserResponse>, AppError> {
    let user = active_current_user(&state, current_user.id).await?;
    let display_name = display_name_or_username(&request.display_name, &user.username);
    let Some(user) = state
        .store
        .users()
        .update_display_name(user.id, display_name, Utc::now())
        .await?
    else {
        return Err(AppError::status(StatusCode::UNAUTHORIZED, "user not found"));
    };
    if user.status != "active" {
        return Err(AppError::status(
            StatusCode::UNAUTHORIZED,
            "user is disabled",
        ));
    }

    Ok(Json(UpdateCurrentUserResponse {
        user: user_summary(&user),
    }))
}

pub(crate) async fn change_password(
    State(state): State<AppState>,
    Extension(current_user): Extension<CurrentUser>,
    Json(request): Json<ChangeCurrentUserPasswordRequest>,
) -> Result<StatusCode, AppError> {
    validate_password(&request.new_password)?;
    let user = active_current_user(&state, current_user.id).await?;
    if !verify_password(&request.current_password, &user.password_hash)? {
        return Err(AppError::status(
            StatusCode::UNAUTHORIZED,
            "invalid current password",
        ));
    }

    let now = Utc::now();
    let password_hash = hash_password(&request.new_password)?;
    let Some(user) = state
        .store
        .users()
        .update_password_hash_and_revoke_refresh_tokens(user.id, password_hash, now)
        .await?
    else {
        return Err(AppError::status(StatusCode::UNAUTHORIZED, "user not found"));
    };
    if user.status != "active" {
        return Err(AppError::status(
            StatusCode::UNAUTHORIZED,
            "user is disabled",
        ));
    }
    Ok(StatusCode::NO_CONTENT)
}

pub(crate) async fn logout(
    State(state): State<AppState>,
    Json(request): Json<RefreshTokenRequest>,
) -> Result<StatusCode, AppError> {
    state
        .store
        .refresh_tokens()
        .revoke(&request.refresh_token, Utc::now())
        .await?;
    Ok(StatusCode::NO_CONTENT)
}

pub(crate) async fn auth_middleware(
    State(state): State<AppState>,
    mut request: HttpRequest<axum::body::Body>,
    next: Next,
) -> Result<AxumResponse, AppError> {
    let Some(header) = request.headers().get(AUTHORIZATION) else {
        return Err(AppError::status(
            StatusCode::UNAUTHORIZED,
            "missing bearer token",
        ));
    };
    let header = header
        .to_str()
        .map_err(|_| AppError::status(StatusCode::UNAUTHORIZED, "invalid bearer token"))?;
    let Some(token) = header.strip_prefix("Bearer ") else {
        return Err(AppError::status(
            StatusCode::UNAUTHORIZED,
            "invalid bearer token",
        ));
    };
    let current_user = state.auth.verify_access_token(token)?;
    request.extensions_mut().insert(current_user);
    Ok(next.run(request).await)
}

impl AuthService {
    pub(crate) async fn load_or_create(
        store: &Store,
        configured_secret: Option<&str>,
    ) -> anyhow::Result<Self> {
        if let Some(secret) = configured_secret
            && !secret.trim().is_empty()
        {
            return Ok(Self {
                jwt_secret: secret.to_string(),
            });
        }

        let existing = store.settings().get_json("jwt_secret").await?;
        if let Some(secret) = existing.and_then(|value| value.as_str().map(str::to_string))
            && !secret.trim().is_empty()
        {
            return Ok(Self { jwt_secret: secret });
        }
        let secret = generate_secret();
        store
            .settings()
            .upsert_json(
                "jwt_secret",
                serde_json::json!(secret),
                Some("JWT signing secret".to_string()),
            )
            .await?;
        Ok(Self { jwt_secret: secret })
    }

    pub(crate) fn development() -> Self {
        Self {
            jwt_secret: "doro-development-jwt-secret-change-before-production".to_string(),
        }
    }

    pub(crate) fn issue_access_token(
        &self,
        user: &StoredUser,
        issued_at: DateTime<Utc>,
    ) -> anyhow::Result<(String, DateTime<Utc>)> {
        let expires_at = issued_at + ChronoDuration::days(1);
        let claims = Claims {
            sub: user.id.to_string(),
            username: user.username.clone(),
            role: user.role.clone(),
            iat: issued_at.timestamp(),
            exp: expires_at.timestamp(),
            jti: Uuid::new_v4().to_string(),
            typ: "access".to_string(),
        };
        let token = jsonwebtoken::encode(
            &Header::default(),
            &claims,
            &EncodingKey::from_secret(self.jwt_secret.as_bytes()),
        )?;
        Ok((token, expires_at))
    }

    pub(crate) fn verify_access_token(&self, token: &str) -> Result<CurrentUser, AppError> {
        let mut validation = Validation::default();
        validation.validate_exp = true;
        let data = jsonwebtoken::decode::<Claims>(
            token,
            &DecodingKey::from_secret(self.jwt_secret.as_bytes()),
            &validation,
        )
        .map_err(|_| AppError::status(StatusCode::UNAUTHORIZED, "invalid bearer token"))?;
        if data.claims.typ != "access" {
            return Err(AppError::status(
                StatusCode::UNAUTHORIZED,
                "invalid bearer token",
            ));
        }
        let id = doro_store::parse_uuid(&data.claims.sub)
            .map_err(|_| AppError::status(StatusCode::UNAUTHORIZED, "invalid bearer token"))?;
        Ok(CurrentUser {
            id,
            username: data.claims.username,
            role: data.claims.role,
        })
    }
}

pub(crate) async fn issue_token_pair(
    state: &AppState,
    user: StoredUser,
    now: DateTime<Utc>,
) -> Result<AuthTokenResponse, AppError> {
    let refresh_token = generate_refresh_token();
    state
        .store
        .refresh_tokens()
        .create(NewRefreshToken {
            id: Uuid::new_v4(),
            user_id: user.id,
            token: refresh_token.clone(),
            created_at: now,
            expires_at: now + ChronoDuration::days(30),
        })
        .await?;
    let (access_token, expires_at) = state.auth.issue_access_token(&user, now)?;
    Ok(AuthTokenResponse {
        access_token,
        refresh_token,
        expires_at,
        user: user_summary(&user),
    })
}

pub(crate) fn user_summary(user: &StoredUser) -> UserSummary {
    UserSummary {
        id: user.id,
        username: user.username.clone(),
        display_name: user.display_name.clone(),
        role: user.role.clone(),
    }
}

pub(crate) async fn active_current_user(
    state: &AppState,
    user_id: Uuid,
) -> Result<StoredUser, AppError> {
    let Some(user) = state.store.users().find_by_id(user_id).await? else {
        return Err(AppError::status(StatusCode::UNAUTHORIZED, "user not found"));
    };
    if user.status != "active" {
        return Err(AppError::status(
            StatusCode::UNAUTHORIZED,
            "user is disabled",
        ));
    }
    Ok(user)
}

pub(crate) fn validate_username(username: &str) -> Result<(), AppError> {
    let username = username.trim();
    if username.len() < 3 || username.len() > 64 {
        return Err(AppError::status(
            StatusCode::BAD_REQUEST,
            "invalid username",
        ));
    }
    if !username
        .chars()
        .all(|character| character.is_ascii_alphanumeric() || matches!(character, '_' | '-' | '.'))
    {
        return Err(AppError::status(
            StatusCode::BAD_REQUEST,
            "invalid username",
        ));
    }
    Ok(())
}

pub(crate) fn validate_password(password: &str) -> Result<(), AppError> {
    if password.chars().count() < 10 {
        return Err(AppError::status(
            StatusCode::BAD_REQUEST,
            "password is too short",
        ));
    }
    Ok(())
}

pub(crate) fn display_name_or_username(display_name: &str, username: &str) -> String {
    let display_name = display_name.trim();
    if display_name.is_empty() {
        username.trim().to_string()
    } else {
        display_name.to_string()
    }
}

pub(crate) fn hash_password(password: &str) -> anyhow::Result<String> {
    let salt = SaltString::generate(&mut OsRng);
    Ok(Argon2::default()
        .hash_password(password.as_bytes(), &salt)
        .map_err(|error| anyhow::anyhow!("failed to hash password: {error}"))?
        .to_string())
}

pub(crate) fn verify_password(password: &str, password_hash: &str) -> anyhow::Result<bool> {
    let parsed_hash = PasswordHash::new(password_hash)
        .map_err(|error| anyhow::anyhow!("invalid password hash: {error}"))?;
    Ok(Argon2::default()
        .verify_password(password.as_bytes(), &parsed_hash)
        .is_ok())
}

pub(crate) fn generate_refresh_token() -> String {
    let mut bytes = [0_u8; 32];
    OsRng.fill_bytes(&mut bytes);
    format!("doro_refresh_{}", URL_SAFE_NO_PAD.encode(bytes))
}

pub(crate) fn generate_enrollment_token() -> String {
    let mut bytes = [0_u8; 32];
    OsRng.fill_bytes(&mut bytes);
    format!("doro_enroll_{}", URL_SAFE_NO_PAD.encode(bytes))
}

pub(crate) fn generate_secret() -> String {
    let mut bytes = [0_u8; 32];
    OsRng.fill_bytes(&mut bytes);
    URL_SAFE_NO_PAD.encode(bytes)
}

#[cfg(test)]
pub(crate) fn default_capabilities() -> Vec<AgentCapability> {
    vec![
        AgentCapability {
            name: doro_protocol::CapabilityName::MetricsRead,
            risk: CapabilityRisk::Low,
            description: "Read CPU, memory, disk, and load metrics".to_string(),
        },
        AgentCapability {
            name: doro_protocol::CapabilityName::LogsRead,
            risk: CapabilityRisk::Low,
            description: "Read service and task logs".to_string(),
        },
        AgentCapability {
            name: doro_protocol::CapabilityName::ShellExecute,
            risk: CapabilityRisk::High,
            description: "Execute shell commands with approval".to_string(),
        },
    ]
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::agent_streams::AgentStreamRegistry;
    use crate::logs::LogHub;
    use crate::state::AppState;
    use axum::Extension;
    use axum::response::IntoResponse;
    use sea_orm::DatabaseBackend;
    use sea_orm::MockDatabase;
    use sea_orm::MockExecResult;

    #[test]
    fn default_capabilities_include_high_risk_shell() {
        assert!(
            default_capabilities()
                .iter()
                .any(|capability| capability.name == CapabilityName::ShellExecute)
        );
    }

    #[test]
    fn validates_usernames_and_passwords() {
        assert!(validate_username("admin.user-1").is_ok());
        assert!(validate_username("ad").is_err());
        assert!(validate_username("admin user").is_err());
        assert!(validate_password("1234567890").is_ok());
        assert!(validate_password("short").is_err());
    }

    #[test]
    fn password_hash_verifies_only_matching_password() -> anyhow::Result<()> {
        let hash = hash_password("correct-password")?;

        assert!(verify_password("correct-password", &hash)?);
        assert!(!verify_password("wrong-password", &hash)?);

        Ok(())
    }

    #[tokio::test]
    async fn me_returns_persisted_display_name() -> anyhow::Result<()> {
        let user = stored_user_for_test("admin", "Control Owner", "hash", "active");
        let state = app_state_for_auth_test(
            MockDatabase::new(DatabaseBackend::Postgres)
                .append_query_results([[user_model_for_test(&user)]])
                .into_connection(),
        );

        let response = me(
            State(state),
            Extension(CurrentUser {
                id: user.id,
                username: user.username.clone(),
                role: user.role.clone(),
            }),
        )
        .await
        .map_err(|error| anyhow::anyhow!("{:?}", error.into_response()))?;

        assert_eq!(response.0.user.display_name, "Control Owner");
        Ok(())
    }

    #[tokio::test]
    async fn update_me_persists_display_name() -> anyhow::Result<()> {
        let user = stored_user_for_test("admin", "Admin", "hash", "active");
        let mut updated_user = user.clone();
        updated_user.display_name = "Home Operator".to_string();
        let state = app_state_for_auth_test(
            MockDatabase::new(DatabaseBackend::Postgres)
                .append_query_results([[user_model_for_test(&user)]])
                .append_query_results([[user_model_for_test(&user)]])
                .append_query_results([[user_model_for_test(&updated_user)]])
                .into_connection(),
        );

        let response = update_me(
            State(state),
            Extension(CurrentUser {
                id: user.id,
                username: user.username.clone(),
                role: user.role.clone(),
            }),
            Json(UpdateCurrentUserRequest {
                display_name: " Home Operator ".to_string(),
            }),
        )
        .await
        .map_err(|error| anyhow::anyhow!("{:?}", error.into_response()))?;

        assert_eq!(response.0.user.display_name, "Home Operator");
        Ok(())
    }

    #[tokio::test]
    async fn change_password_rejects_wrong_current_password() -> anyhow::Result<()> {
        let password_hash = hash_password("correct-password")?;
        let user = stored_user_for_test("admin", "Admin", &password_hash, "active");
        let state = app_state_for_auth_test(
            MockDatabase::new(DatabaseBackend::Postgres)
                .append_query_results([[user_model_for_test(&user)]])
                .into_connection(),
        );

        let error = change_password(
            State(state),
            Extension(CurrentUser {
                id: user.id,
                username: user.username.clone(),
                role: user.role.clone(),
            }),
            Json(ChangeCurrentUserPasswordRequest {
                current_password: "wrong-password".to_string(),
                new_password: "replacement-password".to_string(),
            }),
        )
        .await
        .expect_err("wrong current password should fail");

        assert_eq!(error_status(error), StatusCode::UNAUTHORIZED);
        Ok(())
    }

    #[tokio::test]
    async fn change_password_rejects_short_new_password() -> anyhow::Result<()> {
        let user = stored_user_for_test("admin", "Admin", "hash", "active");
        let state =
            app_state_for_auth_test(MockDatabase::new(DatabaseBackend::Postgres).into_connection());

        let error = change_password(
            State(state),
            Extension(CurrentUser {
                id: user.id,
                username: user.username.clone(),
                role: user.role.clone(),
            }),
            Json(ChangeCurrentUserPasswordRequest {
                current_password: "correct-password".to_string(),
                new_password: "short".to_string(),
            }),
        )
        .await
        .expect_err("short new password should fail");

        assert_eq!(error_status(error), StatusCode::BAD_REQUEST);
        Ok(())
    }

    #[tokio::test]
    async fn change_password_updates_hash_and_revokes_refresh_tokens() -> anyhow::Result<()> {
        let password_hash = hash_password("correct-password")?;
        let user = stored_user_for_test("admin", "Admin", &password_hash, "active");
        let mut updated_user = user.clone();
        updated_user.password_hash = hash_password("replacement-password")?;
        let state = app_state_for_auth_test(
            MockDatabase::new(DatabaseBackend::Postgres)
                .append_query_results([[user_model_for_test(&user)]])
                .append_query_results([[user_model_for_test(&user)]])
                .append_query_results([[user_model_for_test(&updated_user)]])
                .append_exec_results([mock_exec_result()])
                .into_connection(),
        );

        let status = change_password(
            State(state),
            Extension(CurrentUser {
                id: user.id,
                username: user.username.clone(),
                role: user.role.clone(),
            }),
            Json(ChangeCurrentUserPasswordRequest {
                current_password: "correct-password".to_string(),
                new_password: "replacement-password".to_string(),
            }),
        )
        .await
        .map_err(|error| anyhow::anyhow!("{:?}", error.into_response()))?;

        assert_eq!(status, StatusCode::NO_CONTENT);
        Ok(())
    }

    #[test]
    fn jwt_access_token_round_trips_current_user() -> anyhow::Result<()> {
        let auth = AuthService {
            jwt_secret: "test-secret".to_string(),
        };
        let user = StoredUser {
            id: Uuid::new_v4(),
            username: "admin".to_string(),
            display_name: "Admin".to_string(),
            password_hash: "hash".to_string(),
            role: "admin".to_string(),
            status: "active".to_string(),
        };
        let (token, expires_at) = auth.issue_access_token(&user, Utc::now())?;
        let current_user = auth
            .verify_access_token(&token)
            .map_err(|error| anyhow::anyhow!("{error:?}"))?;

        assert_eq!(current_user.id, user.id);
        assert_eq!(current_user.username, "admin");
        assert_eq!(current_user.role, "admin");
        assert!(expires_at > Utc::now());

        Ok(())
    }

    fn app_state_for_auth_test(connection: sea_orm::DatabaseConnection) -> AppState {
        AppState {
            store: Store::from_connection(connection, DatabaseBackend::Postgres),
            auth: AuthService::development(),
            agent_streams: AgentStreamRegistry::default(),
            logs: LogHub::default(),
            control_plane_environment: ControlPlaneEnvironment {
                hostname: "test-host".to_string(),
                os_version: "unknown".to_string(),
                kernel_version: "unknown".to_string(),
                architecture: "unknown".to_string(),
                host_address: "127.0.0.1".to_string(),
                booted_at: None,
                uptime_seconds: 0,
            },
        }
    }

    fn stored_user_for_test(
        username: &str,
        display_name: &str,
        password_hash: &str,
        status: &str,
    ) -> StoredUser {
        StoredUser {
            id: Uuid::new_v4(),
            username: username.to_string(),
            display_name: display_name.to_string(),
            password_hash: password_hash.to_string(),
            role: "admin".to_string(),
            status: status.to_string(),
        }
    }

    fn user_model_for_test(user: &StoredUser) -> doro_store::entities::users::Model {
        doro_store::entities::users::Model {
            id: user.id,
            username: user.username.clone(),
            display_name: user.display_name.clone(),
            password_hash: user.password_hash.clone(),
            role: user.role.clone(),
            status: user.status.clone(),
            created_at: Utc::now().into(),
            updated_at: Utc::now().into(),
            last_login_at: None,
        }
    }

    fn mock_exec_result() -> MockExecResult {
        MockExecResult {
            last_insert_id: 0,
            rows_affected: 1,
        }
    }

    fn error_status(error: AppError) -> StatusCode {
        error.into_response().status()
    }
}
