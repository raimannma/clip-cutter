use itertools::Itertools;
use kdam::tqdm;
use log::debug;
use rand::prelude::StdRng;
use rand::SeedableRng;
use std::time::Duration;

pub(crate) fn get_offset(
    offsets1: &[Duration],
    offsets2: &[Duration],
    min_offset: u64,
) -> Option<u64> {
    debug!("Finding best offset from {:?} to {:?}", offsets1, offsets2);
    let offsets1 = offsets1
        .iter()
        .map(|x| x.as_millis() as u32)
        .collect::<Vec<_>>();
    let offsets2 = offsets2
        .iter()
        .map(|x| x.as_millis() as u32)
        .collect::<Vec<_>>();

    find_best_offset_semi_statistically(&offsets1, &offsets2, min_offset)
}

fn find_best_offset_semi_statistically(
    offsets1: &[u32],
    offsets2: &[u32],
    min_offset: u64,
) -> Option<u64> {
    let mut removers = vec![];
    let max = if offsets1.len() > offsets2.len() {
        offsets1.len() - offsets2.len()
    } else {
        2
    };
    for _ in 0..max {
        removers.push(tpe::TpeOptimizer::new(
            tpe::histogram_estimator(),
            tpe::categorical_range(offsets1.len() + 1).unwrap(),
        ));
    }
    let max = if offsets2.len() > offsets1.len() {
        offsets2.len() - offsets1.len()
    } else {
        2
    };
    let mut removers2 = vec![];
    for _ in 0..max {
        removers2.push(tpe::TpeOptimizer::new(
            tpe::histogram_estimator(),
            tpe::categorical_range(offsets2.len() + 1).unwrap(),
        ));
    }
    let mut best_offset = None;
    let mut smallest_error = None;
    let mut rng = StdRng::from_seed(Default::default());
    for _ in tqdm!(0..10000, desc = "Semi Statistic") {
        let remove_values = removers
            .iter_mut()
            .map(|x| x.ask(&mut rng).unwrap())
            .map(|x| x as usize)
            .collect::<Vec<_>>();
        let remove_values2 = removers2
            .iter_mut()
            .map(|x| x.ask(&mut rng).unwrap())
            .map(|x| x as usize)
            .collect::<Vec<_>>();
        let tmp_offsets1 = offsets1
            .iter()
            .enumerate()
            .filter(|(i, _)| !remove_values.contains(i))
            .map(|(_, x)| *x)
            .collect::<Vec<_>>();
        let tmp_offsets2 = offsets2
            .iter()
            .enumerate()
            .filter(|(i, _)| !remove_values2.contains(i))
            .map(|(_, x)| *x)
            .collect::<Vec<_>>();
        let offset = match find_best_offset_statistically(&tmp_offsets1, &tmp_offsets2, min_offset)
        {
            Some(o) => o,
            None => continue,
        };
        let (offset, error) = offset;
        for (remover, x) in removers.iter_mut().zip(remove_values.iter()) {
            remover.tell(*x as f64, error as f64).unwrap();
        }
        for (remover, x) in removers2.iter_mut().zip(remove_values2.iter()) {
            remover.tell(*x as f64, error as f64).unwrap();
        }
        if smallest_error.is_none() || error < smallest_error.unwrap() {
            smallest_error = Some(error);
            best_offset = Some(offset)
        }
    }
    debug!("Best offset: {:?}", best_offset);
    debug!("Smallest error: {:?}", smallest_error);
    best_offset
}

fn find_best_offset_statistically(
    offsets1: &[u32],
    offsets2: &[u32],
    min_offset: u64,
) -> Option<(u64, u64)> {
    offsets1
        .iter()
        .cartesian_product(offsets2.iter())
        .map(|(o1, o2)| *o1 as i32 - *o2 as i32)
        .map(|o| o as u64)
        .filter(|offset| *offset < 250000)
        .filter(|offset| *offset > min_offset)
        .filter_map(|offset| get_error(offset, offsets1, offsets2).map(|error| (offset, error)))
        .min_by_key(|(_, error)| *error)
        .and_then(|(offset, error)| (error < 200).then_some((offset, error)))
}

fn get_error(offset: u64, offsets1: &[u32], offsets2: &[u32]) -> Option<u64> {
    let errors = offsets2
        .iter()
        .map(|o| *o as u64)
        .map(|x| x + offset)
        .map(|x| x as i64 - get_closest_offset(x, offsets1) as i64)
        .filter(|x| *x > 0)
        .map(|x| x.unsigned_abs())
        .collect::<Vec<_>>();
    (!errors.is_empty()).then(|| errors.iter().sum::<u64>() / errors.len() as u64)
}

fn get_closest_offset(offset: u64, offsets: &[u32]) -> u32 {
    *offsets
        .iter()
        .min_by_key(|x| (offset as i64 - **x as i64).abs())
        .unwrap()
}
