use crate::events::event::{MatchEvent, MatchEventBuilder};
use itertools::Itertools;
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::time::Duration;
use valorant_api_official::response_types::matchdetails_v1::{MatchDetailsV1, RoundResult};

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
pub(crate) struct PlantEvent {
    pub(crate) plant_time: Duration,
    pub(crate) planter: String,
}

impl From<RoundResult> for PlantEvent {
    fn from(round: RoundResult) -> Self {
        let kills = round
            .player_stats
            .into_iter()
            .flat_map(|ps| ps.kills)
            .collect_vec();
        let round_start_time = kills
            .iter()
            .map(|k| k.time_since_game_start_millis - k.time_since_round_start_millis)
            .sum::<u64>()
            / kills.len() as u64;
        Self {
            plant_time: Duration::from_millis(round_start_time + round.plant_round_time.unwrap()),
            planter: round.bomb_planter.unwrap(),
        }
    }
}

impl MatchEventBuilder for PlantEvent {
    fn build_events(valo_match: &MatchDetailsV1) -> Vec<Box<Self>> {
        valo_match
            .round_results
            .as_ref()
            .cloned()
            .unwrap_or_default()
            .into_iter()
            .filter(|r| r.bomb_planter.is_some() && r.bomb_defuser.is_none())
            .filter(|r| r.plant_round_time.is_some() && r.defuse_round_time.is_none())
            .map(Self::from)
            .map(Box::new)
            .collect()
    }
}

impl MatchEvent for PlantEvent {
    fn category(&self, _: &HashSet<String>) -> String {
        "Plant".to_string()
    }

    async fn name_postfix(&self, _: &MatchDetailsV1) -> String {
        "plant".to_string()
    }

    fn game_time_interval(&self) -> (Duration, Duration) {
        (self.plant_time - Duration::from_secs(30), self.plant_time)
    }

    fn is_from_puuids(&self, puuids: &HashSet<String>) -> bool {
        puuids.contains(&self.planter)
    }

    fn is_against_puuids(&self, puuids: &HashSet<String>) -> bool {
        !puuids.contains(&self.planter)
    }
}
