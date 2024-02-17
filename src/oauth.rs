use anyhow::{anyhow, Result};
use chrono::Utc;
use oauth2::basic::{BasicClient, BasicTokenType};
use oauth2::reqwest::async_http_client;
use oauth2::{
    AuthUrl, AuthorizationCode, ClientId, ClientSecret, CsrfToken, EmptyExtraTokenFields,
    PkceCodeChallenge, RedirectUrl, RefreshToken, Scope, StandardTokenResponse,
    TokenResponse, TokenUrl,
};
use serde::{Deserialize, Serialize};
use std::fs::File;
use std::io::{self, BufReader, BufWriter, Write};
use std::path::Path;
use url::Url;

#[derive(Serialize, Deserialize)]
pub struct ClientToken {
    pub refresh_token: String,
    pub access_token: String,
    pub expiration_date: i64,
}

impl TryFrom<StandardTokenResponse<EmptyExtraTokenFields, BasicTokenType>> for ClientToken {
    type Error = anyhow::Error;
    fn try_from(
        token_response: StandardTokenResponse<EmptyExtraTokenFields, BasicTokenType>,
    ) -> Result<Self> {
        let current_time_millis = Utc::now().timestamp_millis();
        let expires_in = token_response
            .expires_in()
            .ok_or(anyhow!("missing expiry"))?;
        Ok(ClientToken {
            refresh_token: token_response
                .refresh_token()
                .ok_or(anyhow!("missing refresh token"))?
                .secret()
                .to_string(),
            access_token: token_response.access_token().secret().to_string(),
            expiration_date: current_time_millis + expires_in.as_millis() as i64,
        })
    }
}

fn get_query_param(
    param: &str,
    query_pairs: url::form_urlencoded::Parse<'_>,
) -> anyhow::Result<String> {
    let result: String = query_pairs
        .into_iter()
        .find_map(|(key, value)| if key == param { Some(value) } else { None })
        .ok_or(anyhow::anyhow!(
            "unable to find param in provided redirect url"
        ))?
        .trim()
        .to_string();

    Ok(result)
}

pub async fn initialize_token(
    client_id: &str,
    client_secret: &str,
    auth_url: &str,
    token_url: &str,
    redirect_url: &str,
) -> Result<ClientToken> {
    // initialize the oauth client
    let client = BasicClient::new(
        ClientId::new(client_id.to_string()),
        Some(ClientSecret::new(client_secret.to_string())),
        AuthUrl::new(auth_url.to_string())?,
        Some(TokenUrl::new(token_url.to_string())?),
    )
    .set_redirect_uri(RedirectUrl::new(redirect_url.to_string())?);

    // generate a challenge - mal only supports plain
    let (pkce_challenge, pkce_verifier) = PkceCodeChallenge::new_random_plain();

    // get the authorization url
    let (auth_url, csrf_token) = client
        .authorize_url(CsrfToken::new_random)
        .add_scope(Scope::new("read".to_string()))
        .add_scope(Scope::new("write".to_string()))
        .set_pkce_challenge(pkce_challenge)
        .url();

    // have the user go to the authorization url
    println!("Open this authorization url in a browser: {}", auth_url);

    // parse the authorization code from the redirect url
    print!("Copy the redirect url here: ");
    io::stdout().flush()?;
    let mut redirect_url = String::new();
    io::stdin().read_line(&mut redirect_url)?;
    let parsed_url = Url::parse(&redirect_url)?;
    let query_pairs: url::form_urlencoded::Parse<'_> = parsed_url.query_pairs();
    let code: String = get_query_param("code", query_pairs)?;
    let state: String = get_query_param("state", query_pairs)?;

    // exchange the code for a token
    let token_result: StandardTokenResponse<EmptyExtraTokenFields, oauth2::basic::BasicTokenType> =
        client
            .exchange_code(AuthorizationCode::new(code))
            .set_pkce_verifier(pkce_verifier)
            .request_async(async_http_client)
            .await?;

    ClientToken::try_from(token_result)
}

pub fn load_client_token(token_json_path: &str) -> anyhow::Result<ClientToken> {
    let file = File::open("token.json")?;
    let reader = BufReader::new(file);
    let client_token: ClientToken = serde_json::from_reader(reader)?;
    Ok(client_token)
}

pub async fn refresh_token(
    client_id: &str,
    client_secret: &str,
    auth_url: &str,
    token_url: &str,
    client_token: ClientToken,
) -> Result<ClientToken> {
    // initialize the oauth client
    let client = BasicClient::new(
        ClientId::new(client_id.to_string()),
        Some(ClientSecret::new(client_secret.to_string())),
        AuthUrl::new(auth_url.to_string())?,
        Some(TokenUrl::new(token_url.to_string())?),
    );

    // exchange the refresh token for a new one
    let token = RefreshToken::new(client_token.refresh_token);
    let token_result = client
        .exchange_refresh_token(&token)
        .request_async(async_http_client)
        .await?;

    ClientToken::try_from(token_result)
}

pub async fn load_or_refresh_token(
    client_id: &str,
    client_secret: &str,
    auth_url: &str,
    token_url: &str,
    redirect_url: &str,
    token_path: &str,
) -> Result<ClientToken> {
    // generate a new token from scratch, since there's no stored tokens
    let mut client_token: ClientToken;
    let current_time_ms = Utc::now().timestamp_millis();
    if !Path::new(token_path).exists() {
        client_token =
            initialize_token(client_id, client_secret, auth_url, token_url, redirect_url).await?;
    }
    // reuse the existing token stored in the token file
    else {
        let file = File::open(token_path)?;
        let reader = BufReader::new(file);
        client_token = serde_json::from_reader(reader)?;
    }

    // the client token has expired! generate a new one from scratch
    let current_time_millis = Utc::now().timestamp_millis();
    let five_days_millis = 1000 * 60 * 60 * 24 * 5;
    if client_token.expiration_date <= current_time_millis {
        client_token =
            initialize_token(client_id, client_secret, auth_url, token_url, redirect_url).await?;
    }
    // the client token is close to expiration. refresh it
    else if current_time_millis - client_token.expiration_date >= five_days_millis {
        client_token =
            refresh_token(client_id, client_secret, auth_url, token_url, client_token).await?;
    }

    // save the client token to disk so that it can be reused
    let file = File::create(token_path)?;
    let writer = BufWriter::new(file);
    serde_json::to_writer_pretty(writer, &client_token)?;

    Ok(client_token)
}
