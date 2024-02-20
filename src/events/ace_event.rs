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
pub(crate) struct AceEvent {
    pub(crate) kill_events: Vec<KillEvent>,
}

impl MatchEventBuilder for AceEvent {
    fn build_events(valo_match: &MatchDetailsV1) -> Vec<Box<Self>> {
        valo_match
            .round_results
            .clone()
            .unwrap_or_default()
            .into_iter()
            .filter(|round| round.round_ceremony == "CeremonyAce")
            .flat_map(|r| r.player_stats)
            .filter(|ps| ps.kills.len() >= 5)
            .map(|ps| AceEvent {
                kill_events: ps
                    .kills
                    .into_iter()
                    .map(KillEvent::from)
                    .sorted_by_key(|ke| ke.game_time)
                    .collect_vec(),
            })
            .map(Box::new)
            .collect_vec()
    }
}

impl AceEvent {
    pub(crate) fn get_kill_agent(&self, valo_match: &MatchDetailsV1) -> Option<String> {
        valorant::get_agent(valo_match, &self.kill_events[0].killer)
            .and_then(|uuid| valorant::get_agent_name(uuid).ok())
    }

    pub(crate) fn get_death_agents(&self, valo_match: &MatchDetailsV1) -> Vec<String> {
        self.kill_events
            .iter()
            .flat_map(|ke| valorant::get_agent(valo_match, &ke.victim))
            .map(valorant::get_agent_name)
            .filter_map(|r| r.as_ref().ok().cloned())
            .collect()
    }

    fn kill_count_postfix(&self) -> String {
        format!("{}k", self.kill_events.len())
    }

    fn weapon_postfix(&self) -> Option<String> {
        self.kill_events
            .first()?
            .finishing_damage
            .damage_item
            .to_lowercase()
            .parse()
            .ok()
            .and_then(|uuid| get_weapon_name(uuid).ok())
            .map(|w| w.to_lowercase())
    }
}

impl MatchEvent for AceEvent {
    fn category(&self, _: &HashSet<String>) -> String {
        "Ace".to_string()
    }

    fn name_postfix(&self, valo_match: &MatchDetailsV1) -> String {
        [
            self.get_kill_agent(valo_match),
            self.get_death_agents(valo_match).join("_").into(),
            self.kill_count_postfix().into(),
            self.weapon_postfix(),
        ]
        .into_iter()
        .flatten()
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
