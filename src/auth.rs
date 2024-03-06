use std::{
    sync::{Arc, Mutex},
    time::Duration,
};

use const_format::concatcp;
use dotenv_codegen::dotenv;
use poem_openapi::{param::Query, ApiResponse, OpenApi};
use reqwest::Client;
use serde::{Deserialize, Serialize};
use serde_json::from_str;
use tracing::{error, info};

const DOMAIN: &str = "localhost";

pub struct AuthApi {
    client: Client,
    redirect: String,
}

impl AuthApi {
    pub fn new() -> Self {
        info!("{:#?}", Self::MICROSOFT);
        Self {
            client: Client::new(),
            redirect: microsoft_redirect_string(),
        }
    }
}

pub(super) fn microsoft_redirect_string() -> String {
    type Api = AuthApi;

    #[derive(Serialize)]
    struct MicrosoftRequestOptions {
        redirect_uri: String,
        client_id: String,
        access_type: String,
        response_type: String,
        scope: String,
    }

    let options = MicrosoftRequestOptions {
        redirect_uri: Api::MICROSOFT.callback_url.to_string(),
        client_id: Api::MICROSOFT.id.to_string(),
        access_type: "offline".to_string(),
        response_type: "code".to_string(),
        scope: "https://graph.microsoft.com/user.read".to_string(),
    };

    let serialized = serde_qs::to_string(&options).expect("It decided to fail");
    format!("{}?{serialized}", Api::MICROSOFT.auth_url)
}

#[OpenApi]
impl AuthApi {
    const MICROSOFT: MicrosoftOauthConst = MicrosoftOauthConst {
        auth_url: concatcp!(
            "https://login.microsoftonline.com/",
            "common",
            "/oauth2/v2.0/authorize"
        ),
        token_url: concatcp!(
            "https://login.microsoftonline.com/",
            dotenv!("MICROSOFT_CLIENT_TENANT"),
            "/oauth2/v2.0/token",
        ),
        id: dotenv!("MICROSOFT_CLIENT_ID"),
        secret: dotenv!("MICROSOFT_CLIENT_SECRET"),
        tenant: dotenv!("MICROSOFT_CLIENT_TENANT"),
        callback_url: concatcp!("http://", DOMAIN, "/api/microsoft/callback"),
    };

    #[oai(path = "/microsoft", method = "get")]
    async fn microsoft_redirect(&self) -> OAuthRedirectResponse {
        OAuthRedirectResponse::SuccessfulRedirect(self.redirect.clone())
    }

    #[oai(path = "/microsoft/callback", method = "get")]
    async fn ms_callback_req(
        &self,
        code: Query<String>,
        session_state: Query<String>,
    ) -> OAuthCallbackResponse {
        info!("{}", session_state.0);
        self.ms_callback(code.0).await
    }

    pub async fn ms_callback(&self, code: String) -> OAuthCallbackResponse {
        #[derive(Serialize)]
        struct MicrosoftCallbackMessage {
            code: String,
            client_id: String,
            client_secret: String,
            redirect_uri: String,
            grant_type: String,
            scope: String,
        }

        let to_send = MicrosoftCallbackMessage {
            code,
            client_id: Self::MICROSOFT.id.to_string(),
            client_secret: Self::MICROSOFT.secret.to_string(),
            redirect_uri: Self::MICROSOFT.callback_url.to_string(),
            grant_type: "authorization_code".to_string(),
            scope: "https://graph.microsoft.com/.default".to_string(),
        };

        let serialized = serde_qs::to_string(&to_send).unwrap();
        info!("its requesting time");
        self.request_user(Self::MICROSOFT.token_url, serialized)
            .await
    }

    pub(super) async fn request_user(&self, root: &str, query: String) -> OAuthCallbackResponse {
        let req = self
            .client
            .post(root)
            .header("Content-Type", "application/x-www-form-urlencoded")
            .body(query);

        let res = match req.timeout(Duration::from_secs(10)).send().await {
            Ok(it) => it,
            Err(err) => {
                error!("Error sending: {}", err);
                return OAuthCallbackResponse::AuthenticationError;
            }
        };

        info!("IT SURVIVED 1");

        match res.error_for_status_ref() {
            Ok(_res) => (),
            Err(err) => {
                error!("Error: {}\n\nBody: {}", err, res.text().await.unwrap());
                return OAuthCallbackResponse::AuthenticationError;
            }
        };
        info!("IT SURVIVED");

        let text = match res.text().await {
            Ok(it) => it,
            Err(err) => {
                error!("Error when getting body: {}", err);
                return OAuthCallbackResponse::AuthenticationError;
            }
        };

        // info!("IT SURVIVED AGAIN {}", text);

        let data: GoogleAuthResponse = match from_str(&text) {
            Ok(it) => it,
            Err(err) => {
                error!("Error when parsing google data: {}", err);
                return OAuthCallbackResponse::AuthenticationError;
            }
        };

        info!("{:#?}", data);

        // TODO Figure out verification and such
        /*
        let validation = Validation::new(jsonwebtoken::Algorithm::RS256);
        validation.insecure_disable_signature_validation();
        */

        OAuthCallbackResponse::SuccessfullyAuthenticated(concatcp!("http://", DOMAIN).to_string())
    }
}

#[derive(Debug, Deserialize)]
pub struct GoogleAuthResponse {
    pub access_token: String,
    pub expires_in: i64,
    // pub refresh_in: String,
    pub scope: String,
    pub token_type: String,
    #[serde(rename = "id_token")]
    pub jwt: String,
}

#[derive(Debug)]
struct MicrosoftOauthConst {
    auth_url: &'static str,
    token_url: &'static str,
    id: &'static str,
    secret: &'static str,
    callback_url: &'static str,
    tenant: &'static str,
}

#[derive(ApiResponse)]
pub enum OAuthCallbackResponse {
    /// When everything goes right and the user successfully authenticates themselves
    #[oai(status = "301")]
    SuccessfullyAuthenticated(#[oai(header = "Location")] String),
    /// When something went wrong during authentication on the server side
    #[oai(status = "500")]
    AuthenticationError,
}

#[derive(ApiResponse)]
pub enum OAuthRedirectResponse {
    #[oai(status = "302")]
    SuccessfulRedirect(#[oai(header = "Location")] String),
}

