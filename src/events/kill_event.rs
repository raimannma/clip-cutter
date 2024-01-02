use crate::events::event::{MatchEvent, MatchEventBuilder};
use crate::valorant;
use crate::valorant::get_weapon_name;
use itertools::Itertools;
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::time::Duration;
use uuid::Uuid;
use valorant_api_official::response_types::matchdetails_v1::{
    KillFinishingDamage, MatchDetailsV1, PlayerRoundKill,
};

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
pub(crate) struct KillEvent {
    pub(crate) game_time: Duration,
    pub(crate) killer: String,
    pub(crate) victim: String,
    pub(crate) finishing_damage: KillFinishingDamage,
}

impl From<PlayerRoundKill> for KillEvent {
    fn from(kill: PlayerRoundKill) -> Self {
        Self {
            game_time: Duration::from_millis(kill.time_since_game_start_millis),
            killer: kill.killer,
            victim: kill.victim,
            finishing_damage: kill.finishing_damage,
        }
    }
}

impl MatchEventBuilder for KillEvent {
    fn build_events(valo_match: &MatchDetailsV1) -> Vec<Box<Self>> {
        valo_match
            .round_results
            .as_ref()
            .cloned()
            .unwrap_or_default()
            .iter()
            .cloned()
            .flat_map(|r| r.player_stats)
            .flat_map(|ps| ps.kills)
            .map(Self::from)
            .map(Box::new)
            .collect()
    }
}

impl KillEvent {
    pub(crate) async fn get_kill_agent(&self, valo_match: &MatchDetailsV1) -> Option<String> {
        let agent_uuid = self.get_agent_uuid(valo_match, &self.killer);
        match agent_uuid {
            Some(agent_uuid) => valorant::get_agent_name(agent_uuid).await.ok(),
            None => None,
        }
    }

    pub(crate) async fn get_death_agent(&self, valo_match: &MatchDetailsV1) -> Option<String> {
        let agent_uuid = self.get_agent_uuid(valo_match, &self.victim);
        match agent_uuid {
            Some(agent_uuid) => valorant::get_agent_name(agent_uuid).await.ok(),
            None => None,
        }
    }

    fn get_agent_uuid(&self, valo_match: &MatchDetailsV1, puuid: &str) -> Option<Uuid> {
        valo_match
            .players
            .iter()
            .find(|p| p.puuid == puuid)
            .and_then(|p| p.character_id)
    }

    fn damage_item_postfix(&self) -> Option<String> {
        let damage_item = self.finishing_damage.damage_item.to_lowercase();
        if damage_item.contains("ability") || damage_item.contains("primary") {
            return Some("ability".to_string());
        }
        if damage_item.contains("ultimate") {
            return Some("ult".to_string());
        }
        get_weapon_name(damage_item.parse().ok()?)
            .ok()
            .map(|w| w.to_lowercase())
    }
}

impl MatchEvent for KillEvent {
    fn category(&self, puuids: &HashSet<String>) -> String {
        if self.is_from_puuids(puuids) {
            return "Kill".to_string();
        }
        if self.is_against_puuids(puuids) {
            return "Death".to_string();
        }
        "Other".to_string()
    }

    async fn name_postfix(&self, valo_match: &MatchDetailsV1) -> String {
        [
            self.get_kill_agent(valo_match).await,
            self.get_death_agent(valo_match).await,
            self.damage_item_postfix(),
        ]
        .iter()
        .flatten()
        .cloned()
        .join("_")
    }

    fn game_time_interval(&self) -> (Duration, Duration) {
        (self.game_time, self.game_time)
    }

    fn is_from_puuids(&self, puuids: &HashSet<String>) -> bool {
        puuids.contains(&self.killer)
    }

    fn is_against_puuids(&self, puuids: &HashSet<String>) -> bool {
        puuids.contains(&self.victim)
    }
}
