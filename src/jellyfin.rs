use std::collections::HashMap;

use anyhow::{anyhow, Context, Result};
use reqwest::Response;
use serde::{Deserialize, Serialize};

pub struct JellyfinApi {
    host: String,
    token: String,
    client: reqwest::Client,
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
struct ItemsResponse {
    items: Vec<Item>,
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
struct User {
    name: String,
    id: String,
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct Item {
    pub id: String,
    #[serde(rename = "Type")]
    pub media_type: String,
    pub index_number: Option<i32>,
    pub parent_index_number: Option<i32>,
    pub name: String,
    pub season_name: Option<String>,
    pub series_name: Option<String>,
    pub series_id: Option<String>,
    pub is_folder: bool,
    pub user_data: UserData,
}

pub struct Episode {
    pub id: String,
    pub number: i32,
    pub name: String,
    pub season_number: i32,
    pub series_name: String,
    pub tvdb_id: i32,
    pub watched: bool,
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct UserData {
    pub played: bool,
    pub key: String,
}

impl JellyfinApi {
    pub fn new(host: &str, token: &str) -> JellyfinApi {
        let client = reqwest::Client::new();
        JellyfinApi {
            host: host.to_string(),
            token: token.to_string(),
            client,
        }
    }

    async fn get(&self, route: &str, params: Option<HashMap<&str, String>>) -> Result<Response> {
        let url = format!("{}{}", self.host, route);
        let mut request_builder = self.client.get(url).header("X-Emby-Token", &self.token);
        if let Some(p) = params {
            request_builder = request_builder.query(&p);
        }
        let response = request_builder.send().await?;
        Ok(response)
    }

    pub async fn get_user_id(&self, username: &str) -> Result<Option<String>> {
        let response = self.get("/Users", None).await?;
        let text = response.text().await?;
        let users_response: Vec<User> = serde_json::from_str(&text)?;
        for user in users_response {
            if user.name == username {
                return Ok(Some(user.id));
            }
        }
        Ok(None)
    }

    pub async fn get_episodes(&self, user_id: &str) -> Result<Vec<Episode>> {
        let items = self.get_items(user_id, None).await?;
        let mut series_tvdb: HashMap<String, String> = HashMap::new();
        let mut episodes: Vec<Episode> = vec![];

        for item in items.iter() {
            if item.media_type == "Series" {
                series_tvdb.insert(item.id.clone(), item.user_data.key.clone());
            }
        }

        for item in items {
            if item.media_type == "Episode" {
                if item.index_number.is_none() {
                    continue;
                }
                let series_name = item.series_name.ok_or(anyhow!("episode missing series"))?;
                let index_number: i32 =
                    item.index_number.ok_or(anyhow!("episode missing number"))?;
                let season_number = item
                    .parent_index_number
                    .ok_or(anyhow!("episode missing season number"))?;
                let series_id = item.series_id.ok_or(anyhow!("episode missing series id"))?;
                let tvdb_id = series_tvdb
                    .get(&series_id)
                    .ok_or(anyhow!("unable to get tvdb id for episode"))?;
                episodes.push(Episode {
                    id: item.id,
                    number: index_number,
                    name: item.name,
                    season_number,
                    series_name,
                    watched: item.user_data.played,
                    tvdb_id: tvdb_id.clone().parse()?,
                });
            }
        }
        Ok(episodes)
    }

    pub async fn get_latest_episodes(
        &self,
        user_id: &str,
    ) -> anyhow::Result<HashMap<i32, Episode>> {
        // get all episodes
        let episodes = self.get_episodes(user_id).await?;

        // get the latest season and episode watched for each series
        let mut status: HashMap<i32, Episode> = HashMap::new();
        episodes.into_iter().for_each(|episode| {
            if !episode.watched {
                return;
            }
            let tvdb_id = episode.tvdb_id;
            if let Some(other) = status.get(&tvdb_id) {
                if episode.season_number > other.season_number
                    || episode.season_number == other.season_number && episode.number > other.number
                {
                    status.insert(tvdb_id, episode);
                }
            } else {
                status.insert(tvdb_id, episode);
            }
        });

        Ok(status)
    }

    pub async fn get_items(&self, user_id: &str, parent_id: Option<&str>) -> Result<Vec<Item>> {
        let mut media: Vec<Item> = vec![];
        let mut frontier: Vec<Option<String>> = vec![parent_id.map(|s| s.to_string())];
        while !frontier.is_empty() {
            // build the params
            let mut params: HashMap<&str, String> = HashMap::new();
            params.insert("userId", user_id.to_string());
            params.insert("enableUserData", "true".to_string());
            if let Some(Some(id)) = frontier.pop() {
                params.insert("parentId", id);
            }
            // get all items under this root
            let response: Response = self.get("/Items", Some(params)).await?;
            let text: String = response.text().await?;
            let items_response: ItemsResponse =
                serde_json::from_str(&text).context("unable to parse items")?;
            for item in items_response.items {
                if item.is_folder {
                    frontier.push(Some(item.id.clone()));
                }
                media.push(item);
            }
        }
        Ok(media)
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;
    use wiremock::matchers::{method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    use super::*;
    #[tokio::test]
    async fn test_get_user_id() -> anyhow::Result<()> {
        let server = MockServer::start().await;
        let jellyfin_client = JellyfinApi::new(&server.uri(), "token");
        let user_id = "123";

        Mock::given(method("GET"))
            .and(path("/Users"))
            .respond_with(ResponseTemplate::new(200).set_body_string(format!(
                "[{{\"Id\": \"{}\", \"Name\": \"alyosha\"}}]",
                user_id
            )))
            .mount(&server)
            .await;

        let result = jellyfin_client.get_user_id("alyosha").await?;
        assert_eq!(result, Some("123".to_string()));

        Ok(())
    }

    #[tokio::test]
    async fn test_get_episodes() -> anyhow::Result<()> {
        let server = MockServer::start().await;
        let jellyfin_client = JellyfinApi::new(&server.uri(), "token");
        let user_id = "123";
        let data = json!([
            {
                "Id": "14",
                "Type": "Series",
                "Name": "test_series",
                "IsFolder": true,
                "UserData": {
                    "Key": "42",
                    "Played": false,
                }
            },
            {
                "Id": "15",
                "Type": "Episode",
                "Name": "test_episode",
                "IsFolder": false,
                "IndexNumber": 8, // episode 8
                "ParentIndexNumber": 2, // season 2
                "SeriesName": "test_series",
                "ParentId": "14",
                "SeriesId": "14",
                "UserData": {
                    "Played": true,
                    "Key": "some_other_not_useful_id"
                }
            }
        ]);

        Mock::given(method("GET"))
            .and(path("/Items"))
            .respond_with(move |request: &wiremock::Request| {
                let parent_id = request.url.query_pairs().find(|(key, _)| key == "parentId");
                if parent_id.is_some() {
                    return ResponseTemplate::new(200).set_body_json(json!({ "Items": [data[1]] }));
                } else {
                    return ResponseTemplate::new(200).set_body_json(json!({ "Items": [data[0]] }));
                }
            })
            .mount(&server)
            .await;

        let result = jellyfin_client.get_episodes(user_id).await?;
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].tvdb_id, 42);
        assert!(result[0].season_number == 2);

        Ok(())
    }

    #[tokio::test]
    async fn test_get_latest_episodes() -> anyhow::Result<()> {
        let server = MockServer::start().await;
        let jellyfin_client = JellyfinApi::new(&server.uri(), "token");
        let user_id = "123";
        let data = json!([
            {
                "Id": "14",
                "Type": "Series",
                "Name": "test_series",
                "IsFolder": true,
                "UserData": {
                    "Key": "42",
                    "Played": false,
                }
            },
            {
                "Id": "15",
                "Type": "Episode",
                "Name": "test_episode",
                "IsFolder": false,
                "IndexNumber": 42, // episode 42
                "ParentIndexNumber": 1, // season 1
                "SeriesName": "test_series",
                "ParentId": "14",
                "SeriesId": "14",
                "UserData": {
                    "Played": true,
                    "Key": "some_other_not_useful_id"
                }
            },

            {
                "Id": "15",
                "Type": "Episode",
                "Name": "test_episode",
                "IsFolder": false,
                "IndexNumber": 8, // episode 8
                "ParentIndexNumber": 2, // season 2
                "SeriesName": "test_series",
                "ParentId": "14",
                "SeriesId": "14",
                "UserData": {
                    "Played": true,
                    "Key": "some_other_not_useful_id"
                }
            },
            {
                "Id": "16",
                "Type": "Episode",
                "Name": "test_episode",
                "IsFolder": false,
                "IndexNumber": 9, // episode 8
                "ParentIndexNumber": 2, // season 2
                "SeriesName": "test_series",
                "ParentId": "14",
                "SeriesId": "14",
                "UserData": {
                    "Played": true,
                    "Key": "some_other_not_useful_id"
                }
            },
            {
                "Id": "17",
                "Type": "Episode",
                "Name": "test_episode",
                "IsFolder": false,
                "IndexNumber": 10, // episode 8
                "ParentIndexNumber": 2, // season 2
                "SeriesName": "test_series",
                "ParentId": "14",
                "SeriesId": "14",
                "UserData": {
                    "Played": false,
                    "Key": "some_other_not_useful_id"
                }
            },
        ]);

        Mock::given(method("GET"))
            .and(path("/Items"))
            .respond_with(move |request: &wiremock::Request| {
                let parent_id = request.url.query_pairs().find(|(key, _)| key == "parentId");
                if parent_id.is_some() {
                    return ResponseTemplate::new(200)
                        .set_body_json(json!({ "Items": [data[1], data[2], data[3]] }));
                } else {
                    return ResponseTemplate::new(200).set_body_json(json!({ "Items": [data[0]] }));
                }
            })
            .mount(&server)
            .await;

        let result = jellyfin_client.get_latest_episodes(user_id).await?;
        assert_eq!(result.len(), 1);
        assert_eq!(result[&42].number, 9);
        assert_eq!(result[&42].season_number, 2);
        Ok(())
    }
}
