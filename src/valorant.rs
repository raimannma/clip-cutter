use cached::proc_macro::cached;
use lazy_static::lazy_static;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::path::Path;
use std::process::ExitStatus;
use std::str::FromStr;
use time::OffsetDateTime;
use uuid::Uuid;
use valorant_api_official::enums::region::Region;
use valorant_api_official::errors::response_error::RequestError;
use valorant_api_official::response_types::matchdetails_v1::{MatchDetailsV1, PlayerRoundKill};
use valorant_api_official::response_types::matchlists_v1::MatchListsEntry;
use valorant_api_official::utils::credentials_manager::CredentialsManager;

use crate::twitch;
use cached::UnboundCache;
use log::info;
use std::time::Duration;

const MAX_MATCH_LENGTH: u64 = 90 * 60 * 1000;

lazy_static! {
    static ref API_KEY: String = std::env::var("RIOT_API_KEY").expect("Failed to get api key");
}

pub async fn find_valorant_matches_by_players(
    puuids: &HashSet<String>,
    interval: (OffsetDateTime, OffsetDateTime),
) -> Result<Vec<MatchDetailsV1>, RequestError> {
    let mut all_matches = vec![];
    for puuid in puuids {
        all_matches.extend(get_valorant_matches_by_player(puuid, interval.0, interval.1).await?);
    }
    Ok(all_matches)
}

pub(crate) fn get_match_length(valo_match: &MatchDetailsV1) -> u64 {
    valo_match
        .match_info
        .game_length_millis
        .unwrap_or(MAX_MATCH_LENGTH)
}

async fn get_valorant_matches_by_player(
    puuid: &str,
    start: OffsetDateTime,
    end: OffsetDateTime,
) -> Result<Vec<MatchDetailsV1>, RequestError> {
    info!("Searching valorant matches for {}", puuid);
    let http_client = Client::new();
    let credentials_manager = CredentialsManager {
        api_key: API_KEY.clone(),
    };

    let region = get_region(&credentials_manager, &http_client, puuid).await?;
    let matches = get_matches(&http_client, &credentials_manager, puuid, region).await?;

    let matches: Vec<&MatchListsEntry> = matches
        .iter()
        .filter(|m| start < m.game_start_time_millis && m.game_start_time_millis < end)
        .collect();

    let mut matches_data = vec![];
    for valo_match in matches {
        matches_data.push(
            get_match_details(
                &http_client,
                &credentials_manager,
                region,
                valo_match.match_id,
            )
            .await,
        );
    }
    Ok(matches_data)
}

async fn get_match_details(
    http_client: &Client,
    credentials_manager: &CredentialsManager,
    region: Region,
    match_id: Uuid,
) -> MatchDetailsV1 {
    let save_path = Path::new("matches/").join(format!("{}.json", match_id));
    if save_path.exists() {
        let match_details = std::fs::read_to_string(save_path).expect("Failed to read match file");
        return serde_json::from_str(&match_details).expect("Failed to parse match file");
    }
    let result = valorant_api_official::get_match_details_v1(
        credentials_manager,
        http_client,
        region,
        &match_id,
    )
    .await
    .expect("Failed to get match details");
    std::fs::create_dir_all(save_path.parent().unwrap()).expect("Failed to create match directory");
    std::fs::write(save_path, serde_json::to_string_pretty(&result).unwrap())
        .expect("Failed to write match file");
    result
}

async fn get_matches(
    http_client: &Client,
    credentials_manager: &CredentialsManager,
    puuid: &str,
    region: Region,
) -> Result<Vec<MatchListsEntry>, RequestError> {
    valorant_api_official::get_match_lists_v1(credentials_manager, http_client, region, puuid)
        .await
        .map(|result| result.history)
}

async fn get_region(
    credentials_manager: &CredentialsManager,
    http_client: &Client,
    puuid: &str,
) -> Result<Region, RequestError> {
    valorant_api_official::get_active_shards_v1(credentials_manager, http_client, puuid)
        .await
        .map(|shard| {
            Region::from_str(&shard.active_shard.to_string()).expect("Failed to get region")
        })
}

pub(crate) async fn get_puuid(riot_id: &str) -> Result<String, RequestError> {
    let http_client = Client::new();
    let credentials_manager = CredentialsManager {
        api_key: API_KEY.clone(),
    };
    let (name, tag) = riot_id.split_once('#').expect("Failed to split riot id");

    valorant_api_official::get_accounts_by_name_v1(&credentials_manager, &http_client, name, tag)
        .await
        .map(|accounts| accounts.puuid)
}

pub(crate) fn get_match_kills(valo_match: &MatchDetailsV1) -> Vec<PlayerRoundKill> {
    valo_match
        .round_results
        .clone()
        .unwrap_or_default()
        .into_iter()
        .flat_map(|r| r.player_stats)
        .flat_map(|p| p.kills)
        .collect()
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
struct APIData<T> {
    data: T,
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
#[serde(rename_all = "camelCase")]
struct AgentData {
    display_name: String,
}

#[cached(
    type = "UnboundCache<String, String>",
    create = "{ UnboundCache::with_capacity(30) }",
    result = true,
    convert = r#"{ format!("{}", agent_uuid) }"#
)]
pub(crate) fn get_agent_name(agent_uuid: Uuid) -> reqwest::Result<String> {
    let url = format!("https://valorant-api.com/v1/agents/{}", agent_uuid);
    Ok(reqwest::blocking::get(url)?
        .json::<APIData<AgentData>>()?
        .data
        .display_name)
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
#[serde(rename_all = "camelCase")]
struct WeaponData {
    display_name: String,
}

#[cached(
    type = "UnboundCache<String, String>",
    create = "{ UnboundCache::with_capacity(30) }",
    result = true,
    convert = r#"{ format!("{}", weapon_uuid) }"#
)]
pub(crate) fn get_weapon_name(weapon_uuid: Uuid) -> reqwest::Result<String> {
    let url = format!("https://valorant-api.com/v1/weapons/{}", weapon_uuid);
    Ok(reqwest::blocking::get(url)?
        .json::<APIData<WeaponData>>()?
        .data
        .display_name)
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
#[serde(rename_all = "camelCase")]
struct MapData {
    display_name: String,
    map_url: String,
}

#[cached(
    type = "UnboundCache<String, Option<String>>",
    create = "{ UnboundCache::with_capacity(30) }",
    result = true,
    convert = r#"{ format!("{}", map_url) }"#
)]
pub(crate) fn get_map_name(map_url: &str) -> reqwest::Result<Option<String>> {
    let url = "https://valorant-api.com/v1/maps";
    Ok(reqwest::blocking::get(url)?
        .json::<APIData<Vec<MapData>>>()?
        .data
        .into_iter()
        .find(|m| m.map_url == map_url)
        .map(|m| m.display_name))
}

pub fn save_match_video(
    match_video_path: &Path,
    vod_id: usize,
    start: Duration,
    end: Duration,
) -> std::io::Result<ExitStatus> {
    twitch::download_vod(vod_id, match_video_path, &start, &end)
}

pub(crate) fn get_agent(valo_match: &MatchDetailsV1, puuid: &str) -> Option<Uuid> {
    valo_match
        .players
        .iter()
        .find(|p| p.puuid == puuid)
        .and_then(|p| p.character_id)
}
