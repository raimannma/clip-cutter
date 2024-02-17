use crate::events::event::{MatchEvent, MatchEventBuilder};
use itertools::Itertools;
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::time::Duration;
use valorant_api_official::response_types::matchdetails_v1::{MatchDetailsV1, RoundResult};

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
pub(crate) struct DefuseEvent {
    pub(crate) plant_time: Duration,
    pub(crate) defuse_time: Duration,
    pub(crate) planter: String,
    pub(crate) defuser: String,
}

impl From<RoundResult> for DefuseEvent {
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
            defuse_time: Duration::from_millis(round_start_time + round.defuse_round_time.unwrap()),
            planter: round.bomb_planter.unwrap(),
            defuser: round.bomb_defuser.unwrap(),
        }
    }
}

impl MatchEventBuilder for DefuseEvent {
    fn build_events(valo_match: &MatchDetailsV1) -> Vec<Box<Self>> {
        valo_match
            .round_results
            .as_ref()
            .cloned()
            .unwrap_or_default()
            .into_iter()
            .filter(|r| r.bomb_planter.is_some() && r.bomb_defuser.is_some())
            .map(Self::from)
            .map(Box::new)
            .collect()
    }
}

impl MatchEvent for DefuseEvent {
    async fn category(&self, _: &HashSet<String>) -> String {
        "Defuse".to_string()
    }

    async fn name_postfix(&self, match_details: &MatchDetailsV1) -> String {
        let kills = match_details
            .round_results
            .as_ref()
            .cloned()
            .unwrap_or_default()
            .iter()
            .flat_map(|r| r.player_stats.clone())
            .flat_map(|ps| ps.kills)
            .filter(|k| k.time_since_game_start_millis < self.defuse_time.as_millis() as u64)
            .filter(|k| k.time_since_game_start_millis > self.plant_time.as_millis() as u64)
            .filter(|k| k.killer == self.defuser)
            .count();
        format!(
            "{}s_{}k",
            (self.defuse_time.as_secs() as i64 - self.plant_time.as_secs() as i64),
            kills
        )
    }

    fn game_time_interval(&self) -> (Duration, Duration) {
        (self.plant_time - Duration::from_secs(4), self.defuse_time)
    }

    fn is_from_puuids(&self, puuids: &HashSet<String>) -> bool {
        puuids.contains(&self.defuser)
    }

    fn is_against_puuids(&self, puuids: &HashSet<String>) -> bool {
        puuids.contains(&self.planter)
    }
}
