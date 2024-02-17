use log::{info, debug};
use mal::MyAnimeListApi;
use mapping::tvdb_id_to_mal_id;

use anyhow::{anyhow, Context};
use jellyfin::JellyfinApi;
use std::env;

mod jellyfin;
mod mal;
mod mapping;
mod oauth;


const MAL_AUTH_URL: &str = "https://myanimelist.net/v1/oauth2/authorize";
const MAL_TOKEN_URL: &str = "https://myanimelist.net/v1/oauth2/token";
const MAL_TOKEN_PATH: &str = "./token.json";


#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    env_logger::init();

    let jellyfin_host = &env::var("JELLYFIN_HOST")?;
    let jellyfin_token = &env::var("JELLYFIN_TOKEN")?;
    let jellyfin_user = &env::var("JELLYFIN_USER")?;

    // initialize the api
    debug!("initializing the jellyfin api");
    let jellyfin_api = JellyfinApi::new(jellyfin_host, jellyfin_token);

    // get the latest episode the user has watched for all series
    debug!("getting the user id");
    let user_id = jellyfin_api
        .get_user_id(jellyfin_user)
        .await?
        .ok_or(anyhow!("user does not exist"))?;
    let latest_episodes = jellyfin_api.get_latest_episodes(&user_id).await?;

    // get a token to access the mal api
    debug!("getting an access token to communicate with the mal api");
    let mal_token = oauth::load_or_refresh_token(
        &env::var("MAL_CLIENT_ID")?,
        &env::var("MAL_CLIENT_SECRET")?,
        MAL_AUTH_URL,
        MAL_TOKEN_URL,
        &env::var("MAL_API_REDIRECT_URL")?,
        MAL_TOKEN_PATH,
    ).await?;

    // initialize the mal api
    let mal_api: MyAnimeListApi = MyAnimeListApi::new(mal_token);

    // for each series, find the mal id. if the user's latest watched on
    // jellyfin is greater than the latest watch on MAL, update the user's
    for (tvdb_id, episode) in latest_episodes {
        let mal_id = tvdb_id_to_mal_id(tvdb_id, episode.season_number)?;
        let mal_latest_episode_number = mal_api.get_latest_episode_number(mal_id).await?;
        if episode.number > mal_latest_episode_number {
            info!("setting latest episode of series {} (mal-id: {}) to {}", episode.series_name, mal_id, episode.number);
            mal_api
                .set_latest_episode_number(mal_id, episode.number)
                .await?;
        }
    }

    Ok(())
}
