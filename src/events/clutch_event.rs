use crate::events::event::{MatchEvent, MatchEventBuilder};
use crate::events::kill_event::KillEvent;
use crate::valorant;
use itertools::Itertools;
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::time::Duration;
use valorant_api_official::response_types::matchdetails_v1::MatchDetailsV1;

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
pub(crate) struct ClutchEvent {
    pub(crate) clutcher: String,
    pub(crate) kill_events: Vec<KillEvent>,
}

impl MatchEventBuilder for ClutchEvent {
    fn build_events(valo_match: &MatchDetailsV1) -> Vec<Box<Self>> {
        let mut clutches = vec![];
        for round in valo_match.round_results.clone().unwrap_or_default() {
            if round.round_ceremony != "CeremonyClutch" {
                continue;
            }
            let kill_events = round
                .player_stats
                .iter()
                .cloned()
                .flat_map(|ps| ps.kills)
                .map(KillEvent::from)
                .sorted_by_key(|ke| ke.game_time)
                .collect::<Vec<_>>();
            let clutcher = match kill_events.last() {
                Some(ke) => ke.killer.clone(),
                None => continue,
            };
            let last_killer_team = match valo_match.players.iter().find(|p| p.puuid == clutcher) {
                Some(p) => p.team_id.clone(),
                None => continue,
            };
            if last_killer_team != round.winning_team {
                continue;
            }
            let kill_events = kill_events
                .into_iter()
                .filter(|ke| ke.killer == clutcher)
                .collect::<Vec<_>>();
            clutches.push(Box::new(Self {
                clutcher,
                kill_events,
            }));
        }
        clutches
    }
}

impl ClutchEvent {
    pub(crate) async fn get_kill_agent(&self, valo_match: &MatchDetailsV1) -> Option<String> {
        match valorant::get_agent(valo_match, &self.clutcher) {
            Some(agent_uuid) => valorant::get_agent_name(agent_uuid).await.ok(),
            None => None,
        }
    }
}

impl MatchEvent for ClutchEvent {
    fn category(&self, _: &HashSet<String>) -> String {
        "Clutch".to_string()
    }

    async fn name_postfix(&self, valo_match: &MatchDetailsV1) -> String {
        self.get_kill_agent(valo_match).await.iter().join("_")
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
