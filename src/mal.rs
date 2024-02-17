use std::collections::HashMap;

use anyhow::Result;
use reqwest::header::HeaderMap;
use reqwest::Response;
use serde::Deserialize;
use serde::Serialize;

use crate::oauth::ClientToken;

const MAL_ENDPOINT: &str = "https://api.myanimelist.net/v2";

#[derive(Serialize, Deserialize)]
struct UserAnimeListResponse {
    data: Vec<UserAnimeListDatum>,
}

#[derive(Serialize, Deserialize)]

struct UserAnimeListDatum {
    node: UserAnimeListNode,
    list_status: UserAnimeListStatus,
}

#[derive(Serialize, Deserialize)]
struct UserAnimeListNode {
    id: i32,
    title: String,
}

#[derive(Serialize, Deserialize)]
struct UserAnimeListStatus {
    num_episodes_watched: i32,
}

pub struct MyAnimeListApi {
    pub client: reqwest::Client,
    pub token: ClientToken,
}

enum RequestType {
    Get,
    Patch,
}

impl MyAnimeListApi {
    pub fn new(token: ClientToken) -> MyAnimeListApi {
        MyAnimeListApi {
            client: reqwest::Client::new(),
            token,
        }
    }

    async fn request(
        &self,
        request_type: RequestType,
        route: &str,
        params: Option<HashMap<&str, &str>>,
        form_data: Option<HashMap<&str, String>>,
    ) -> anyhow::Result<Response> {
        let headers: HeaderMap = HeaderMap::new();
        let url = format!("{}{}", MAL_ENDPOINT, route);
        let mut request_builder = match request_type {
            RequestType::Get => self.client.get(url),
            RequestType::Patch => self.client.patch(url),
        };
        request_builder = request_builder.headers(headers);
        if let Some(p) = params {
            request_builder = request_builder.query(&p);
        }
        if let Some(f) = form_data {
            request_builder = request_builder.form(&f);
        }

        let response: Response = request_builder
            .bearer_auth(&self.token.access_token)
            .send()
            .await?;

        Ok(response)
    }

    pub async fn get_latest_episode_number(&self, series_id: i32) -> Result<i32> {
        let mut params: HashMap<&str, &str> = HashMap::new();
        params.insert("limit", "1000");
        params.insert("fields", "list_status");
        let user_anime_list_response = self
            .request(RequestType::Get, "/users/@me/animelist", Some(params), None)
            .await?;
        let text = user_anime_list_response.text().await?;
        let user_anime_list: UserAnimeListResponse = serde_json::from_str(&text)?;
        for datum in user_anime_list.data {
            if datum.node.id == series_id {
                return Ok(datum.list_status.num_episodes_watched);
            }
        }
        Ok(0)
    }

    pub async fn set_latest_episode_number(
        &self,
        series_id: i32,
        episode_number: i32,
    ) -> Result<()> {
        let mut form_data: HashMap<&str, String> = HashMap::new();
        form_data.insert("num_watched_episodes", episode_number.to_string());
        form_data.insert("status", "watching".to_string());
        self.request(
            RequestType::Patch,
            &format!("/anime/{}/my_list_status", series_id),
            None,
            Some(form_data),
        )
        .await?;
        Ok(())
    }
}
