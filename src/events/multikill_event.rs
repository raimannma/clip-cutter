use crate::events::event::{MatchEvent, MatchEventBuilder};
use crate::events::kill_event::KillEvent;
use crate::valorant;
use crate::valorant::get_weapon_name;
use itertools::Itertools;
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::time::Duration;
use valorant_api_official::response_types::matchdetails_v1::MatchDetailsV1;

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
pub(crate) struct MultiKillEvent {
    pub(crate) kill_events: Vec<KillEvent>,
}

impl MatchEventBuilder for MultiKillEvent {
    fn build_events(valo_match: &MatchDetailsV1) -> Vec<Box<Self>> {
        let mut multikills = vec![];
        let puuids = valo_match
            .players
            .iter()
            .map(|p| p.puuid.clone())
            .collect::<Vec<_>>();
        for round in valo_match.round_results.clone().unwrap_or_default() {
            for player in puuids.iter() {
                let mut current_group = vec![];
                let kills = round
                    .clone()
                    .player_stats
                    .iter()
                    .cloned()
                    .flat_map(|ps| ps.kills)
                    .map(KillEvent::from)
                    .sorted_by_key(|ke| ke.game_time)
                    .collect::<Vec<_>>();
                for kill in kills {
                    if kill.victim == *player {
                        if current_group.len() > 2 {
                            multikills.push(Box::new(Self {
                                kill_events: current_group.clone(),
                            }));
                        }
                        current_group.clear();
                    } else if kill.killer == *player {
                        current_group.push(kill);
                    }
                }
                if current_group.len() > 2 {
                    multikills.push(Box::new(Self {
                        kill_events: current_group.clone(),
                    }));
                }
            }
        }
        multikills
    }
}

impl MultiKillEvent {
    pub(crate) async fn get_kill_agent(&self, valo_match: &MatchDetailsV1) -> Option<String> {
        let agent_uuid = valorant::get_agent(valo_match, &self.kill_events[0].killer);
        match agent_uuid {
            Some(agent_uuid) => valorant::get_agent_name(agent_uuid).await.ok(),
            None => None,
        }
    }

    pub(crate) async fn get_death_agents(&self, valo_match: &MatchDetailsV1) -> Vec<String> {
        futures::future::join_all(
            self.kill_events
                .iter()
                .flat_map(|ke| valorant::get_agent(valo_match, &ke.victim))
                .map(valorant::get_agent_name),
        )
        .await
        .iter()
        .filter_map(|r| r.as_ref().ok().cloned())
        .collect()
    }

    fn kill_count_postfix(&self) -> String {
        format!("{}k", self.kill_events.len())
    }

    async fn weapon_postfix(&self) -> Option<String> {
        get_weapon_name(
            self.kill_events
                .first()?
                .finishing_damage
                .damage_item
                .to_lowercase()
                .parse()
                .ok()?,
        )
        .await
        .ok()
        .map(|w| w.to_lowercase())
    }
}

impl MatchEvent for MultiKillEvent {
    async fn category(&self, _: &HashSet<String>) -> String {
        "Multikill".to_string()
    }

    async fn name_postfix(&self, valo_match: &MatchDetailsV1) -> String {
        [
            self.get_kill_agent(valo_match).await,
            self.get_death_agents(valo_match).await.join("_").into(),
            self.kill_count_postfix().into(),
            self.weapon_postfix().await,
        ]
        .iter()
        .flatten()
        .cloned()
        .join("_")
    }

    fn game_time_interval(&self) -> (Duration, Duration) {
        let sorted_events = self
            .kill_events
            .iter()
            .map(|ke| ke.game_time)
            .sorted()
            .collect::<Vec<_>>();
        (sorted_events[0], sorted_events[sorted_events.len() - 1])
    }

    fn is_from_puuids(&self, puuids: &HashSet<String>) -> bool {
        self.kill_events.iter().any(|ke| ke.is_from_puuids(puuids))
    }

    fn is_against_puuids(&self, puuids: &HashSet<String>) -> bool {
        self.kill_events
            .iter()
            .any(|ke| ke.is_against_puuids(puuids))
    }
}
