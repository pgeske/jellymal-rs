use std::{fs::File, io::BufReader};

use anyhow::{anyhow, Result};
use serde::{Deserialize, Serialize};
use serde_xml_rs::from_reader;

#[derive(Serialize, Deserialize)]
struct Anime {
    anidbid: String,
    tvdbid: String,
    defaulttvdbseason: String,
}

#[derive(Serialize, Deserialize)]
struct AnimeList {
    #[serde(rename = "$value")]
    animes: Vec<Anime>,
}

#[derive(Serialize, Deserialize)]
struct OfflineAnime {
    anidb_id: Option<i32>,
    mal_id: Option<i32>,
}

pub fn tvdb_id_to_mal_id(tvdb_id: i32, tvdb_season_number: i32) -> Result<i32> {
    let anidb_id = tvdb_id_to_anidb_id(tvdb_id, tvdb_season_number)?;
    let mal_id = anidb_id_to_mal_id(anidb_id)?;
    Ok(mal_id)
}

fn tvdb_id_to_anidb_id(tvdb_id: i32, tvdb_season_number: i32) -> Result<i32> {
    let f = File::open("anime-list-master.xml")?;
    let reader = BufReader::new(f);
    let anime_list: AnimeList = from_reader(reader)?;
    for anime in anime_list.animes {
        if anime.tvdbid == tvdb_id.to_string()
            && anime.defaulttvdbseason == tvdb_season_number.to_string()
        {
            return Ok(anime.anidbid.parse()?);
        }
    }
    Err(anyhow!("unable to map tvdb to anidb"))
}

fn anidb_id_to_mal_id(anidb_id: i32) -> Result<i32> {
    let f = File::open("anime-list-full.json")?;
    let reader = BufReader::new(f);
    let animes: Vec<OfflineAnime> = serde_json::from_reader(reader)?;
    animes
        .iter()
        .find_map(|anime| {
            if anime.anidb_id == Some(anidb_id) {
                return anime.mal_id;
            }
            None
        })
        .ok_or(anyhow!("unable to map anidb id to mal id"))
}
