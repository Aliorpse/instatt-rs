use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use crate::{
    auth::{azure, firebase},
    error::{Error, Result},
};

const REFRESH_MARGIN: Duration = Duration::from_secs(120);

#[derive(Debug, Clone)]
struct Session {
    firebase_id_token: String,
    firebase_refresh_token: Option<String>,
    azure_refresh_token: Option<String>,
    firebase_id_token_valid_until: Instant,
}

impl Session {
    fn from_firebase(
        session: firebase::FirebaseSession,
        azure_refresh_token: Option<String>,
    ) -> Self {
        Self {
            firebase_id_token: session.id_token,
            firebase_refresh_token: Some(session.refresh_token),
            azure_refresh_token,
            firebase_id_token_valid_until: Instant::now() + Duration::from_secs(session.expires_in),
        }
    }

    fn usable(&self) -> bool {
        Instant::now() + REFRESH_MARGIN < self.firebase_id_token_valid_until
    }
}

pub struct InstattClient {
    pub(crate) http: reqwest::Client,
    student_id: String,
    session: tokio::sync::Mutex<Session>,
    pub(crate) globals: tokio::sync::RwLock<Option<crate::model::Globals>>,
    pub(crate) device_uid: tokio::sync::RwLock<Option<String>>,
    token_refresh_lock: tokio::sync::Mutex<()>,
    device_transition_lock: tokio::sync::Mutex<()>,
}

impl InstattClient {
    pub async fn login_with_auth_url(callback_url: &str) -> Result<Self> {
        Self::login_with_azure_code(&azure::parse_callback(callback_url)?).await
    }

    pub async fn login_with_azure_code(code: &str) -> Result<Self> {
        let http = build_http_client()?;
        let token = azure::exchange_code(&http, code).await?;
        Self::from_azure_token(http, token.access_token, token.refresh_token).await
    }

    pub async fn login_with_azure_access_token(
        access_token: &str,
        refresh_token: Option<&str>,
    ) -> Result<Self> {
        Self::from_azure_token(
            build_http_client()?,
            access_token.to_owned(),
            refresh_token.map(str::to_owned),
        )
        .await
    }

    pub async fn restore_with_firebase_refresh_token(refresh_token: &str) -> Result<Self> {
        let http = build_http_client()?;
        let session = firebase::refresh_id_token(&http, refresh_token).await?;
        let identity = firebase::decode_identity(&session.id_token)?;
        Self::ensure_student(&identity)?;
        Ok(Self::new(
            http,
            identity,
            Session::from_firebase(session, None),
        ))
    }

    async fn from_azure_token(
        http: reqwest::Client,
        access_token: String,
        refresh_token: Option<String>,
    ) -> Result<Self> {
        let custom_token = firebase::exchange_azure_token(&http, &access_token).await?;
        let session = firebase::sign_in_with_custom_token(&http, &custom_token).await?;
        let identity = firebase::decode_identity(&session.id_token)?;
        Self::ensure_student(&identity)?;
        Ok(Self::new(
            http,
            identity,
            Session::from_firebase(session, refresh_token),
        ))
    }

    fn new(http: reqwest::Client, identity: firebase::FirebaseIdentity, session: Session) -> Self {
        Self {
            http,
            student_id: identity.student_id,
            session: tokio::sync::Mutex::new(session),
            globals: tokio::sync::RwLock::new(None),
            device_uid: tokio::sync::RwLock::new(None),
            token_refresh_lock: tokio::sync::Mutex::new(()),
            device_transition_lock: tokio::sync::Mutex::new(()),
        }
    }

    fn ensure_student(identity: &firebase::FirebaseIdentity) -> Result<()> {
        if identity.privilege_type == 0 {
            Ok(())
        } else {
            Err(Error::NotStudentAccount {
                privilege_type: identity.privilege_type,
            })
        }
    }

    pub fn student_id(&self) -> &str {
        &self.student_id
    }

    pub async fn firebase_refresh_token(&self) -> Option<String> {
        self.session.lock().await.firebase_refresh_token.clone()
    }

    pub(crate) async fn bearer(&self) -> Result<String> {
        {
            let session = self.session.lock().await;
            if session.usable() {
                return Ok(session.firebase_id_token.clone());
            }
        }

        let _refresh = self.token_refresh_lock.lock().await;
        {
            let session = self.session.lock().await;
            if session.usable() {
                return Ok(session.firebase_id_token.clone());
            }
        }

        let (firebase_refresh_token, azure_refresh_token) = {
            let session = self.session.lock().await;
            (
                session.firebase_refresh_token.clone(),
                session.azure_refresh_token.clone(),
            )
        };

        if let Some(refresh_token) = firebase_refresh_token {
            match firebase::refresh_id_token(&self.http, &refresh_token).await {
                Ok(next) => {
                    let token = next.id_token.clone();
                    let mut session = self.session.lock().await;
                    *session = Session::from_firebase(next, azure_refresh_token);
                    return Ok(token);
                }
                Err(Error::FirebaseRefreshRejected(_)) => {}
                Err(error) => return Err(error),
            }
        }

        let refresh_token = azure_refresh_token.ok_or(Error::NotAuthenticated)?;
        let azure_token = azure::refresh(&self.http, &refresh_token).await?;
        let azure_refresh = azure_token.refresh_token.or(Some(refresh_token));
        let custom_token =
            firebase::exchange_azure_token(&self.http, &azure_token.access_token).await?;
        let next = firebase::sign_in_with_custom_token(&self.http, &custom_token).await?;
        let token = next.id_token.clone();
        let mut session = self.session.lock().await;
        *session = Session::from_firebase(next, azure_refresh);
        Ok(token)
    }

    pub(crate) async fn device_transition(&self) -> tokio::sync::MutexGuard<'_, ()> {
        self.device_transition_lock.lock().await
    }

    pub(crate) fn unix_seconds() -> u64 {
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_or(0, |duration| duration.as_secs())
    }
}

fn build_http_client() -> Result<reqwest::Client> {
    reqwest::Client::builder()
        .user_agent(concat!("instatt-rs/", env!("CARGO_PKG_VERSION")))
        .timeout(Duration::from_secs(60))
        .build()
        .map_err(Error::HttpClientBuild)
}
