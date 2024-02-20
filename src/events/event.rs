use crate::events::ace_event::AceEvent;
use crate::events::clutch_event::ClutchEvent;
use crate::events::defuse_event::DefuseEvent;
use crate::events::doublekill_event::DoubleKillEvent;
use crate::events::kill_event::KillEvent;
use crate::events::multikill_event::MultiKillEvent;
use crate::events::plant_event::PlantEvent;
use crate::events::retake_event::RetakeEvent;
use serde::Serialize;
use std::collections::HashSet;
use std::fmt::Debug;
use std::time::Duration;
use valorant_api_official::response_types::matchdetails_v1::MatchDetailsV1;

pub(crate) trait MatchEvent: Debug + Clone + Serialize {
    fn category(&self, puuids: &HashSet<String>) -> String;
    fn name_postfix(&self, valo_match: &MatchDetailsV1) -> String;
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
    Clutch(ClutchEvent),
    DoubleKill(DoubleKillEvent),
    Plant(PlantEvent),
    Defuse(DefuseEvent),
    Ace(AceEvent),
    Retake(RetakeEvent),
}

pub(crate) fn build_events(valo_match: &MatchDetailsV1) -> Vec<Event> {
    [
        KillEvent::build_events(valo_match)
            .into_iter()
            .map(|e| Event::Kill(*e))
            .collect::<Vec<_>>(),
        MultiKillEvent::build_events(valo_match)
            .into_iter()
            .map(|e| Event::MultiKill(*e))
            .collect::<Vec<_>>(),
        ClutchEvent::build_events(valo_match)
            .into_iter()
            .map(|e| Event::Clutch(*e))
            .collect::<Vec<_>>(),
        DoubleKillEvent::build_events(valo_match)
            .into_iter()
            .map(|e| Event::DoubleKill(*e))
            .collect::<Vec<_>>(),
        PlantEvent::build_events(valo_match)
            .into_iter()
            .map(|e| Event::Plant(*e))
            .collect::<Vec<_>>(),
        DefuseEvent::build_events(valo_match)
            .into_iter()
            .map(|e| Event::Defuse(*e))
            .collect::<Vec<_>>(),
        AceEvent::build_events(valo_match)
            .into_iter()
            .map(|e| Event::Ace(*e))
            .collect::<Vec<_>>(),
        RetakeEvent::build_events(valo_match)
            .into_iter()
            .map(|e| Event::Retake(*e))
            .collect::<Vec<_>>(),
    ]
    .into_iter()
    .flatten()
    .collect()
}

impl MatchEvent for Event {
    fn category(&self, puuids: &HashSet<String>) -> String {
        match self {
            Event::Kill(e) => e.category(puuids),
            Event::MultiKill(e) => e.category(puuids),
            Event::Clutch(e) => e.category(puuids),
            Event::DoubleKill(e) => e.category(puuids),
            Event::Plant(e) => e.category(puuids),
            Event::Defuse(e) => e.category(puuids),
            Event::Ace(e) => e.category(puuids),
            Event::Retake(e) => e.category(puuids),
        }
    }

    fn name_postfix(&self, valo_match: &MatchDetailsV1) -> String {
        match self {
            Event::Kill(e) => e.name_postfix(valo_match),
            Event::MultiKill(e) => e.name_postfix(valo_match),
            Event::Clutch(e) => e.name_postfix(valo_match),
            Event::DoubleKill(e) => e.name_postfix(valo_match),
            Event::Plant(e) => e.name_postfix(valo_match),
            Event::Defuse(e) => e.name_postfix(valo_match),
            Event::Ace(e) => e.name_postfix(valo_match),
            Event::Retake(e) => e.name_postfix(valo_match),
        }
    }

    fn game_time_interval(&self) -> (Duration, Duration) {
        match self {
            Event::Kill(e) => e.game_time_interval(),
            Event::MultiKill(e) => e.game_time_interval(),
            Event::Clutch(e) => e.game_time_interval(),
            Event::DoubleKill(e) => e.game_time_interval(),
            Event::Plant(e) => e.game_time_interval(),
            Event::Defuse(e) => e.game_time_interval(),
            Event::Ace(e) => e.game_time_interval(),
            Event::Retake(e) => e.game_time_interval(),
        }
    }

    fn is_from_puuids(&self, puuids: &HashSet<String>) -> bool {
        match self {
            Event::Kill(e) => e.is_from_puuids(puuids),
            Event::MultiKill(e) => e.is_from_puuids(puuids),
            Event::Clutch(e) => e.is_from_puuids(puuids),
            Event::DoubleKill(e) => e.is_from_puuids(puuids),
            Event::Plant(e) => e.is_from_puuids(puuids),
            Event::Defuse(e) => e.is_from_puuids(puuids),
            Event::Ace(e) => e.is_from_puuids(puuids),
            Event::Retake(e) => e.is_from_puuids(puuids),
        }
    }

    fn is_against_puuids(&self, puuids: &HashSet<String>) -> bool {
        match self {
            Event::Kill(e) => e.is_against_puuids(puuids),
            Event::MultiKill(e) => e.is_against_puuids(puuids),
            Event::Clutch(e) => e.is_against_puuids(puuids),
            Event::DoubleKill(e) => e.is_against_puuids(puuids),
            Event::Plant(e) => e.is_against_puuids(puuids),
            Event::Defuse(e) => e.is_against_puuids(puuids),
            Event::Ace(e) => e.is_against_puuids(puuids),
            Event::Retake(e) => e.is_against_puuids(puuids),
        }
    }
}
