//! The adaptive request of the goal `pack`: reads the words of the input
//! into the policy and the work budget the service takes, and gives the
//! walk its clock.
//!
//! The core holds no clock, so the deadline lives here, at the edge: an
//! [`Interrupt`] the walk asks before every visit.

use std::str::FromStr;
use std::sync::Arc;
use std::time::{Duration, Instant};

use super::PackInput;
use crate::application::context_pack::{
    Adaptive, Detail, Expansion, Interrupt, Purpose, SelectionPolicy, Stop, WorkBudget,
};

/// The words that ask for the default of a choice.
const AUTO: &str = "auto";

/// A time limit counted from the moment the request was read.
#[derive(Debug)]
struct Deadline {
    started: Instant,
    limit: Duration,
}

impl Interrupt for Deadline {
    fn poll(&self) -> Option<Stop> {
        (self.started.elapsed() >= self.limit).then_some(Stop::DeadlineExceeded)
    }
}

/// The adaptive selection the input asks for, when it asks for one.
///
/// `mode` is what turns it on; the other fields only tune it, so they are
/// refused without it instead of being ignored. A pack of a change takes its
/// seed and its roles from the change, so `mode` is refused with it too.
pub(super) fn adaptive_of(input: &PackInput, is_change: bool) -> Result<Option<Adaptive>, String> {
    let Some(mode) = input.mode.as_deref() else {
        let stray: Vec<&str> = [
            ("detail", input.detail.is_some()),
            ("expansion", input.expansion.is_some()),
            ("max_visits", input.max_visits.is_some()),
            ("time_ms", input.time_ms.is_some()),
        ]
        .into_iter()
        .filter_map(|(name, given)| given.then_some(name))
        .collect();
        if stray.is_empty() {
            return Ok(None);
        }
        return Err(format!(
            "`{}` tune the adaptive selection: pass `mode` (`diagnose`, `refactor` or \
             `architecture`) too.",
            stray.join("`, `")
        ));
    };
    if is_change {
        return Err(
            "`mode` selects around the symbols of a request; a pack of a change takes its \
             roles from the change: do not pass `mode` with `change_scope`."
                .to_string(),
        );
    }
    let mut policy = SelectionPolicy::for_mode(Purpose::from_str(mode)?);
    policy.detail_override = match input.detail.as_deref() {
        None | Some(AUTO) => None,
        Some(word) => Some(Detail::from_str(word)?),
    };
    if let Some(word) = input.expansion.as_deref() {
        policy.expansion = Expansion::from_str(word)?;
    }
    let mut budget = WorkBudget::default();
    if let Some(visits) = input.max_visits {
        if visits == 0 {
            return Err("`max_visits` is 0: the walk needs at least one visit.".to_string());
        }
        budget.max_visits = usize::try_from(visits).unwrap_or(usize::MAX);
    }
    if let Some(millis) = input.time_ms {
        if millis == 0 {
            return Err("`time_ms` is 0: the walk needs some time.".to_string());
        }
        budget.interrupt = Arc::new(Deadline {
            started: Instant::now(),
            limit: Duration::from_millis(millis),
        });
    }
    Ok(Some(Adaptive { policy, budget }))
}

/// What a continuation token of an adaptive pack is bound to: the parameters
/// that change which symbols are selected and in which order.
pub(super) fn token_scope_of(adaptive: &Adaptive, input: &PackInput) -> String {
    let policy = &adaptive.policy;
    format!(
        "adaptive:{}:{}:{}:{}:{}:{}",
        policy.version,
        policy.mode.map_or("custom", Purpose::code),
        policy.detail_override.map_or(AUTO, Detail::code),
        match policy.expansion {
            Expansion::Auto => "auto",
            Expansion::Always => "always",
            Expansion::Never => "never",
        },
        adaptive.budget.max_visits,
        input.time_ms.unwrap_or(0),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn input(mode: Option<&str>) -> PackInput {
        PackInput {
            symbols: vec!["x".to_string()],
            intent: None,
            max_tokens: None,
            tokenizer: None,
            continuation_token: None,
            change_scope: None,
            base: None,
            head: None,
            mode: mode.map(str::to_string),
            detail: None,
            expansion: None,
            max_visits: None,
            time_ms: None,
        }
    }

    #[test]
    fn without_a_mode_the_request_is_the_plain_one() {
        assert!(adaptive_of(&input(None), false).unwrap().is_none());
    }

    #[test]
    fn a_tuning_without_a_mode_is_refused_by_name() {
        let mut stray = input(None);
        stray.detail = Some("full".to_string());
        stray.time_ms = Some(5);

        let error = adaptive_of(&stray, false).unwrap_err();

        assert!(error.contains("`detail`, `time_ms`"), "{error}");
        assert!(error.contains("pass `mode`"), "{error}");
    }

    #[test]
    fn a_mode_is_refused_with_a_change() {
        let error = adaptive_of(&input(Some("diagnose")), true).unwrap_err();

        assert!(error.contains("change_scope"), "{error}");
    }

    #[test]
    fn the_words_of_the_input_become_the_policy() {
        let mut tuned = input(Some("refactor"));
        tuned.detail = Some("signature".to_string());
        tuned.expansion = Some("never".to_string());
        tuned.max_visits = Some(7);

        let adaptive = adaptive_of(&tuned, false).unwrap().unwrap();

        assert_eq!(adaptive.policy.mode, Some(Purpose::Refactor));
        assert_eq!(adaptive.policy.detail_override, Some(Detail::Signature));
        assert_eq!(adaptive.policy.expansion, Expansion::Never);
        assert_eq!(adaptive.budget.max_visits, 7);
        assert!(adaptive.budget.interrupt.poll().is_none());
    }

    #[test]
    fn auto_detail_leaves_the_choice_to_the_role() {
        let mut tuned = input(Some("diagnose"));
        tuned.detail = Some("auto".to_string());

        let adaptive = adaptive_of(&tuned, false).unwrap().unwrap();

        assert_eq!(adaptive.policy.detail_override, None);
    }

    #[test]
    fn words_that_are_not_a_choice_are_refused() {
        assert!(adaptive_of(&input(Some("explore")), false).is_err());
        let mut bad = input(Some("diagnose"));
        bad.expansion = Some("sometimes".to_string());
        assert!(adaptive_of(&bad, false).is_err());
        let mut zero = input(Some("diagnose"));
        zero.max_visits = Some(0);
        assert!(adaptive_of(&zero, false).is_err());
        zero.max_visits = None;
        zero.time_ms = Some(0);
        assert!(adaptive_of(&zero, false).is_err());
    }

    #[test]
    fn a_deadline_that_passed_stops_the_walk_and_one_that_did_not_does_not() {
        let passed = Deadline {
            started: Instant::now() - Duration::from_secs(2),
            limit: Duration::from_secs(1),
        };
        let open = Deadline {
            started: Instant::now(),
            limit: Duration::from_secs(3600),
        };

        assert_eq!(passed.poll(), Some(Stop::DeadlineExceeded));
        assert_eq!(open.poll(), None);
    }

    #[test]
    fn the_token_scope_changes_with_every_parameter_that_changes_the_selection() {
        let base = input(Some("diagnose"));
        let scope =
            |input: &PackInput| token_scope_of(&adaptive_of(input, false).unwrap().unwrap(), input);
        let reference = scope(&base);

        let mut other = input(Some("refactor"));
        assert_ne!(scope(&other), reference);
        other = input(Some("diagnose"));
        other.detail = Some("full".to_string());
        assert_ne!(scope(&other), reference);
        other = input(Some("diagnose"));
        other.expansion = Some("never".to_string());
        assert_ne!(scope(&other), reference);
        other = input(Some("diagnose"));
        other.max_visits = Some(3);
        assert_ne!(scope(&other), reference);
        assert_eq!(scope(&base), reference);
    }
}
