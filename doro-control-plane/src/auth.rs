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
    Extension(current_user): Extension<CurrentUser>,
) -> Json<CurrentUserResponse> {
    Json(CurrentUserResponse {
        user: UserSummary {
            id: current_user.id,
            username: current_user.username.clone(),
            display_name: current_user.username,
            role: current_user.role,
        },
    })
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
}
