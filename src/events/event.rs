use crate::events::clutch_event::Clutch;
use crate::events::kill_event::KillEvent;
use crate::events::multikill_event::MultiKillEvent;
use serde::Serialize;
use std::collections::HashSet;
use std::fmt::Debug;
use std::time::Duration;
use valorant_api_official::response_types::matchdetails_v1::MatchDetailsV1;

pub(crate) trait MatchEvent: Debug + Clone + Serialize {
    fn category(&self, puuids: &HashSet<String>) -> String;
    async fn name_postfix(&self, valo_match: &MatchDetailsV1) -> String;
    fn game_time_interval(&self) -> (Duration, Duration);
    fn is_from_puuids(&self, puuids: &HashSet<String>) -> bool;
    fn is_against_puuids(&self, puuids: &HashSet<String>) -> bool;
}

pub(crate) trait MatchEventBuilder {
    fn build_events(valo_match: &MatchDetailsV1) -> Vec<Box<Self>>;
}

#[derive(Debug, Clone, Serialize)]
#[serde(untagged)]
pub(crate) enum Event {
    Kill(KillEvent),
    MultiKill(MultiKillEvent),
    Clutch(Clutch),
}

pub(crate) fn build_events(valo_match: &MatchDetailsV1) -> Vec<Event> {
    [
        KillEvent::build_events(valo_match)
            .iter()
            .cloned()
            .map(|e| Event::Kill(*e))
            .collect::<Vec<_>>(),
        MultiKillEvent::build_events(valo_match)
            .iter()
            .cloned()
            .map(|e| Event::MultiKill(*e))
            .collect::<Vec<_>>(),
        Clutch::build_events(valo_match)
            .iter()
            .cloned()
            .map(|e| Event::Clutch(*e))
            .collect::<Vec<_>>(),
    ]
    .iter()
    .flatten()
    .cloned()
    .collect()
}

impl MatchEvent for Event {
    fn category(&self, puuids: &HashSet<String>) -> String {
        match self {
            Event::Kill(e) => e.category(puuids),
            Event::MultiKill(e) => e.category(puuids),
            Event::Clutch(e) => e.category(puuids),
        }
    }

    async fn name_postfix(&self, valo_match: &MatchDetailsV1) -> String {
        match self {
            Event::Kill(e) => e.name_postfix(valo_match).await,
            Event::MultiKill(e) => e.name_postfix(valo_match).await,
            Event::Clutch(e) => e.name_postfix(valo_match).await,
        }
    }

    fn game_time_interval(&self) -> (Duration, Duration) {
        match self {
            Event::Kill(e) => e.game_time_interval(),
            Event::MultiKill(e) => e.game_time_interval(),
            Event::Clutch(e) => e.game_time_interval(),
        }
    }

    fn is_from_puuids(&self, puuids: &HashSet<String>) -> bool {
        match self {
            Event::Kill(e) => e.is_from_puuids(puuids),
            Event::MultiKill(e) => e.is_from_puuids(puuids),
            Event::Clutch(e) => e.is_from_puuids(puuids),
        }
    }

    fn is_against_puuids(&self, puuids: &HashSet<String>) -> bool {
        match self {
            Event::Kill(e) => e.is_against_puuids(puuids),
            Event::MultiKill(e) => e.is_against_puuids(puuids),
            Event::Clutch(e) => e.is_against_puuids(puuids),
        }
    }
}
