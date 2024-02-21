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
    pub(crate) shots: Option<(u32, u32, u32)>,
}

impl From<PlayerRoundKill> for KillEvent {
    fn from(kill: PlayerRoundKill) -> Self {
        Self {
            game_time: Duration::from_millis(kill.time_since_game_start_millis),
            killer: kill.killer,
            victim: kill.victim,
            finishing_damage: kill.finishing_damage,
            shots: None,
        }
    }
}

impl From<(PlayerRoundKill, u32, u32, u32)> for KillEvent {
    fn from(kill: (PlayerRoundKill, u32, u32, u32)) -> Self {
        let (kill, headshots, bodyshots, legshots) = kill;
        Self {
            game_time: Duration::from_millis(kill.time_since_game_start_millis),
            killer: kill.killer,
            victim: kill.victim,
            finishing_damage: kill.finishing_damage,
            shots: Some((headshots, bodyshots, legshots)),
        }
    }
}

impl MatchEventBuilder for KillEvent {
    fn build_events(valo_match: &MatchDetailsV1) -> Vec<Box<Self>> {
        let player_stats = valo_match
            .round_results
            .as_ref()
            .cloned()
            .unwrap_or_default()
            .into_iter()
            .flat_map(|r| r.player_stats)
            .collect_vec();
        let mut events = vec![];
        for ps in player_stats {
            for kill in ps.kills {
                let (headshots, bodyshots, legshots) = ps
                    .damage
                    .iter()
                    .filter(|d| d.receiver == kill.victim)
                    .fold((0, 0, 0), |(h, b, l), d| {
                        (h + d.headshots, b + d.bodyshots, l + d.legshots)
                    });
                events.push(Box::new(KillEvent::from((
                    kill, headshots, bodyshots, legshots,
                ))));
            }
        }
        events
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

    async fn damage_item_postfix(&self) -> Option<String> {
        let damage_item = self.finishing_damage.damage_item.to_lowercase();
        if damage_item.contains("ability")
            || damage_item.contains("primary")
            || damage_item.contains("ultimate")
        {
            return None;
        }
        get_weapon_name(damage_item.parse().ok()?)
            .await
            .ok()
            .map(|w| w.to_lowercase())
    }
}

impl MatchEvent for KillEvent {
    async fn category(&self, puuids: &HashSet<String>) -> String {
        let is_from = self.is_from_puuids(puuids);
        let is_against = self.is_against_puuids(puuids);
        if is_from && is_against {
            "Death"
        } else if is_from && self.damage_item_postfix().await.is_some() {
            if let Some((headshots, bodyshots, legshots)) = self.shots {
                let is_sniper = [
                    "a03b24d3-4319-996d-0f8c-94bbfba1dfc7",
                    "5f0aaf7a-4289-3998-d5ff-eb9a5cf7ef5c",
                    "c4883e50-4494-202c-3ec3-6b8a9284f00b",
                ]
                .contains(&self.finishing_damage.damage_item.as_str());
                let is_secondary = self.finishing_damage.is_secondary_fire_mode;
                if headshots + bodyshots + legshots == 1 && is_sniper && is_secondary {
                    return "NoScopeSniper".to_string();
                }
                if headshots == 1 && bodyshots == 0 && legshots == 0 {
                    return "Onetap".to_string();
                }
            }
            return "Kill".to_string();
        } else if is_from && self.damage_item_postfix().await.is_none() {
            "AbilityKill"
        } else if is_against {
            "Death"
        } else {
            "Other"
        }
        .to_string()
    }

    async fn name_postfix(&self, valo_match: &MatchDetailsV1) -> String {
        [
            self.get_kill_agent(valo_match).await,
            self.get_death_agent(valo_match).await,
            self.damage_item_postfix().await,
        ]
        .into_iter()
        .flatten()
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
