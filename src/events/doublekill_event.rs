use crate::events::event::{MatchEvent, MatchEventBuilder};
use crate::events::kill_event::KillEvent;
use crate::valorant;
use crate::valorant::get_weapon_name;
use itertools::Itertools;
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::time::Duration;
use tuple_conv::RepeatedTuple;
use valorant_api_official::response_types::matchdetails_v1::MatchDetailsV1;

const KILL_TIME: u64 = 2;

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
pub(crate) struct DoubleKillEvent {
    pub(crate) kill_events: (KillEvent, KillEvent),
}

impl MatchEventBuilder for DoubleKillEvent {
    fn build_events(valo_match: &MatchDetailsV1) -> Vec<Box<Self>> {
        valo_match
            .players
            .iter()
            .map(|p| p.puuid.clone())
            .flat_map(|player| {
                valo_match
                    .round_results
                    .clone()
                    .unwrap_or_default()
                    .into_iter()
                    .flat_map(|rr| rr.player_stats)
                    .flat_map(|ps| ps.kills)
                    .filter(|k| k.killer == *player)
                    .map(KillEvent::from)
                    .sorted_by_key(|ke| ke.game_time)
                    .tuple_windows()
                    .filter(|(k, k2)| k.game_time + Duration::from_secs(KILL_TIME) >= k2.game_time)
                    .filter(|(k, k2)| {
                        k.finishing_damage.damage_item == k2.finishing_damage.damage_item
                    })
                    .filter(|(k, k2)| {
                        k.finishing_damage.damage_type == k2.finishing_damage.damage_type
                    })
                    .map(|k| DoubleKillEvent { kill_events: k })
                    .map(Box::new)
            })
            .collect_vec()
    }
}

impl DoubleKillEvent {
    pub(crate) async fn get_kill_agent(&self, valo_match: &MatchDetailsV1) -> Option<String> {
        let agent_uuid = valorant::get_agent(valo_match, &self.kill_events.0.killer);
        match agent_uuid {
            Some(agent_uuid) => valorant::get_agent_name(agent_uuid).await.ok(),
            None => None,
        }
    }

    pub(crate) async fn get_death_agents(&self, valo_match: &MatchDetailsV1) -> Vec<String> {
        futures::future::join_all(
            self.kill_events
                .clone()
                .to_vec()
                .into_iter()
                .flat_map(|ke| valorant::get_agent(valo_match, &ke.victim))
                .map(valorant::get_agent_name),
        )
        .await
        .iter()
        .filter_map(|r| r.as_ref().ok().cloned())
        .collect()
    }

    async fn weapon_postfix(&self) -> Option<String> {
        get_weapon_name(
            self.kill_events
                .0
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

impl MatchEvent for DoubleKillEvent {
    async fn category(&self, _: &HashSet<String>) -> String {
        "Doublekill".to_string()
    }

    async fn name_postfix(&self, valo_match: &MatchDetailsV1) -> String {
        [
            self.get_kill_agent(valo_match).await,
            self.get_death_agents(valo_match).await.join("_").into(),
            self.weapon_postfix().await,
        ]
        .into_iter()
        .flatten()
        .join("_")
    }

    fn game_time_interval(&self) -> (Duration, Duration) {
        let sorted_events = self
            .kill_events
            .clone()
            .to_vec()
            .iter()
            .map(|ke| ke.game_time)
            .sorted()
            .collect::<Vec<_>>();
        (sorted_events[0], sorted_events[sorted_events.len() - 1])
    }

    fn is_from_puuids(&self, puuids: &HashSet<String>) -> bool {
        self.kill_events
            .clone()
            .to_vec()
            .iter()
            .any(|ke| ke.is_from_puuids(puuids))
    }

    fn is_against_puuids(&self, puuids: &HashSet<String>) -> bool {
        self.kill_events
            .clone()
            .to_vec()
            .iter()
            .any(|ke| ke.is_against_puuids(puuids))
    }
}
