use crate::events::event::{MatchEvent, MatchEventBuilder};
use itertools::Itertools;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::time::Duration;
use valorant_api_official::enums::team::Team;
use valorant_api_official::response_types::matchdetails_v1::{
    MatchDetailsV1, RoundResult, TeamUnion,
};

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
pub(crate) struct RetakeEvent {
    pub(crate) winners: HashSet<String>,
    pub(crate) losers: HashSet<String>,
    pub(crate) plant_time: Duration,
    pub(crate) defuse_time: Duration,
}

fn is_attacker(round_num: u8, team: &TeamUnion) -> bool {
    if let TeamUnion::Team(t) = team {
        match t {
            Team::RED => is_attacker_red(round_num),
            Team::BLUE => !is_attacker_red(round_num),
            _ => false,
        }
    } else {
        false
    }
}

fn is_attacker_red(round_num: u8) -> bool {
    if round_num < 12 {
        return true;
    }
    if round_num < 24 {
        return false;
    }
    round_num % 2 == 0
}

impl RetakeEvent {
    fn new(valo_match: &MatchDetailsV1, round: &RoundResult) -> Self {
        let (winners, losers) = valo_match
            .players
            .iter()
            .partition::<Vec<_>, _>(|p| round.winning_team == p.team_id);
        let kills = round
            .player_stats
            .iter()
            .flat_map(|ps| ps.kills.iter())
            .collect_vec();
        let round_start_time = kills
            .iter()
            .map(|k| k.time_since_game_start_millis - k.time_since_round_start_millis)
            .sum::<u64>()
            / kills.len() as u64;
        Self {
            winners: winners.iter().map(|p| p.puuid.clone()).collect(),
            losers: losers.iter().map(|p| p.puuid.clone()).collect(),
            plant_time: Duration::from_millis(round_start_time + round.plant_round_time.unwrap()),
            defuse_time: Duration::from_millis(round_start_time + round.defuse_round_time.unwrap()),
        }
    }
}

impl MatchEventBuilder for RetakeEvent {
    fn build_events(valo_match: &MatchDetailsV1) -> Vec<Box<Self>> {
        let mut retake_events = vec![];
        let teams = valo_match
            .players
            .clone()
            .into_iter()
            .map(|p| (p.puuid, p.team_id))
            .collect::<HashMap<_, _>>();
        for round in valo_match.round_results.clone().unwrap_or_default() {
            if round.bomb_planter.is_none()
                || round.bomb_defuser.is_none()
                || round.plant_round_time == round.defuse_round_time
            {
                continue;
            }
            let kills_before_plant = round
                .player_stats
                .iter()
                .flat_map(|ps| ps.kills.iter())
                .filter(|k| k.time_since_round_start_millis < round.plant_round_time.unwrap())
                .collect_vec();
            let attacker_deaths_before_plant = kills_before_plant
                .iter()
                .filter(|k| is_attacker(round.round_num, teams.get(&k.victim).unwrap()))
                .count();
            let defender_deaths_before_plant = kills_before_plant
                .iter()
                .filter(|k| !is_attacker(round.round_num, teams.get(&k.victim).unwrap()))
                .count();
            if attacker_deaths_before_plant <= 1 && defender_deaths_before_plant <= 1 {
                retake_events.push(Box::new(RetakeEvent::new(valo_match, &round)));
            }
        }
        retake_events
    }
}

impl MatchEvent for RetakeEvent {
    fn category(&self, _: &HashSet<String>) -> String {
        "Retake".to_string()
    }

    fn name_postfix(&self, _: &MatchDetailsV1) -> String {
        format!(
            "{}s",
            (self.defuse_time.as_secs() as i64 - self.plant_time.as_secs() as i64)
        )
    }

    fn game_time_interval(&self) -> (Duration, Duration) {
        (self.plant_time - Duration::from_secs(4), self.defuse_time)
    }

    fn is_from_puuids(&self, puuids: &HashSet<String>) -> bool {
        self.winners.iter().any(|w| puuids.contains(w))
    }

    fn is_against_puuids(&self, puuids: &HashSet<String>) -> bool {
        self.losers.iter().any(|l| puuids.contains(l))
    }
}
