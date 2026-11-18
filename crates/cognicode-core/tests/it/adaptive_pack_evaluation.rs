//! The measured comparison of the ways to select context: the name alone
//! (IDF), the graph alone (Personalized PageRank) and the hybrid policy, plus
//! one ablation per signal.
//!
//! The corpus is fixed and generated: a few families of worlds that share a
//! shape —a contract, a primary implementation with helpers of one class,
//! sibling implementations, a consumer, a test and a hub every function
//! logs through— with the relevant set of each request labelled by hand.
//! The numbers below are measured on that corpus and say nothing about code
//! outside it: it is small, synthetic and homogeneous on purpose, so that a
//! change of weights shows up as a change of a number.
//!
//! The tests assert the claims the defaults rest on. A weight or a heuristic
//! the numbers do not support is not kept.
//!
//! What the corpus measured at a budget of 1500 (mean over five families;
//! recall and precision of the labelled relevant set, redundant items
//! delivered per pack):
//!
//! | request  | method          | recall | precision | redundant |
//! |----------|-----------------|--------|-----------|-----------|
//! | anchored | plain pack      | 0.55   | 0.23      | 4.4       |
//! | anchored | IDF only        | 0.50   | 0.32      | 1.2       |
//! | anchored | PPR only        | 0.50   | 0.37      | 2.4       |
//! | anchored | hybrid diagnose | 0.75   | 0.75      | 0.0       |
//! | intent   | plain pack      | 0.60   | 0.32      | 4.4       |
//! | intent   | IDF only        | 0.36   | 0.26      | 3.2       |
//! | intent   | PPR only        | 0.72   | 0.65      | 0.0       |
//! | intent   | hybrid diagnose | 0.88   | 0.88      | 0.0       |
//!
//! Removing one piece of the hybrid, anchored then intent, recall: no PPR
//! 0.75 / 0.80 (and the hub gets in), no impact 0.70 / 0.76, no lexical
//! 0.75 / 0.80 (the hub gets in on intent), no diversity 0.70 / 0.72, no
//! role bonus 0.50 / 0.64. Proximity changes nothing at the defaults on
//! this corpus: it is kept at a low weight and flagged as unproven.
//!
//! The weights of the diagnose mode, the role bonus and the diversity come
//! from the searches below, run on three families and checked on the two
//! the search never saw (mean F1 over both requests and two budgets: the
//! previous defaults 0.557 / 0.623, the kept ones 0.704 / 0.733). The
//! refactor and architecture weights are declared, not searched.

use std::collections::BTreeSet;

use cognicode_core::application::context_pack::{
    Adaptive, Expansion, Purpose, SelectionPolicy, Weights, WorkBudget,
};
use cognicode_core::domain::value_objects::{DependencyType::*, SymbolKind};

use super::adaptive_pack_world::{World, WorldBuilder};

const SUFFIXES: [&str; 6] = ["header", "checksum", "atomic", "index", "audit", "stamp"];

/// One family member: names only, the shape is shared.
struct Spec {
    contract: &'static str,
    verb: &'static str,
    primary: &'static str,
    others: &'static [&'static str],
    helpers: usize,
    consumer: &'static str,
    test: &'static str,
}

const SPECS: [Spec; 5] = [
    Spec {
        contract: "Storage",
        verb: "save",
        primary: "File",
        others: &["Memory", "Cloud"],
        helpers: 4,
        consumer: "backup_job",
        test: "a_file_survives_a_roundtrip",
    },
    Spec {
        contract: "Gateway",
        verb: "charge",
        primary: "Card",
        others: &["Wire", "Voucher", "Wallet"],
        helpers: 5,
        consumer: "settle_invoice",
        test: "a_card_charge_is_idempotent",
    },
    Spec {
        contract: "Tokenizer",
        verb: "scan",
        primary: "Json",
        others: &["Toml"],
        helpers: 3,
        consumer: "parse_manifest",
        test: "scanning_keeps_every_span",
    },
    Spec {
        contract: "Notifier",
        verb: "publish",
        primary: "Email",
        others: &["Sms", "Push"],
        helpers: 6,
        consumer: "announce_release",
        test: "an_email_is_published_once",
    },
    Spec {
        contract: "Scheduler",
        verb: "enqueue",
        primary: "Cron",
        others: &["Delayed", "Manual"],
        helpers: 4,
        consumer: "plan_nightly_run",
        test: "a_cron_job_is_enqueued_on_time",
    },
];

fn snake(pascal: &str) -> String {
    let mut out = String::new();
    for (at, c) in pascal.chars().enumerate() {
        if c.is_uppercase() && at > 0 {
            out.push('_');
        }
        out.extend(c.to_lowercase());
    }
    out
}

/// A world and the labels of the requests over it.
struct Case {
    world: World,
    intent: String,
    /// The anchor of the anchored scenario.
    seed: &'static str,
    /// The symbols a reader of a defect in the seed needs, the seed apart.
    relevant: BTreeSet<String>,
    /// The symbols that repeat what the seed already shows: helpers of its
    /// class and the same method of the sibling implementations.
    redundant: BTreeSet<String>,
    /// The hub every function logs through and its callers.
    noise: BTreeSet<String>,
}

fn build(spec: &Spec) -> Case {
    let c = spec.contract;
    let c_snake = snake(c);
    let verb = spec.verb;
    let helper_names: Vec<String> = SUFFIXES[..spec.helpers]
        .iter()
        .map(|suffix| format!("{verb}_{suffix}"))
        .collect();

    let trait_src = format!(
        "/// The contract of every {c_snake}.\npub trait {c} {{\n    fn {verb}(&self, key: &str, bytes: &[u8]) -> Result<(), Error>;\n    fn load(&self, key: &str) -> Result<Vec<u8>, Error>;\n}}\n"
    );
    let mut primary_src = format!(
        "pub struct {p}{c} {{\n    root: PathBuf,\n}}\n\nimpl {p}{c} {{\n",
        p = spec.primary
    );
    for name in &helper_names {
        primary_src.push_str(&format!(
            "    fn {name}(&self, key: &str) -> Result<(), Error> {{\n        let target = self.root.join(key);\n        record(&target)?;\n        Ok(())\n    }}\n\n"
        ));
    }
    primary_src.push_str(&format!(
        "}}\n\nimpl {c} for {p}{c} {{\n    fn {verb}(&self, key: &str, bytes: &[u8]) -> Result<(), Error> {{\n        let mut payload = bytes.to_vec();\n",
        p = spec.primary
    ));
    for name in &helper_names {
        primary_src.push_str(&format!("        self.{name}(key)?;\n"));
    }
    primary_src.push_str(
        "        write_all(&self.root, key, &payload)\n    }\n\n    fn load(&self, key: &str) -> Result<Vec<u8>, Error> {\n        read_all(&self.root, key)\n    }\n}\n",
    );

    let consumer_src = format!(
        "pub fn {consumer}(target: &{p}{c}, keys: &[String]) -> Result<(), Error> {{\n    for key in keys {{\n        target.{verb}(key, b\"payload\")?;\n    }}\n    audit_trail(\"{consumer}\");\n    Ok(())\n}}\n\n#[test]\nfn {test}() {{\n    let target = {p}{c}::open(\"/tmp/case\");\n    target.{verb}(\"k\", b\"v\").unwrap();\n    assert_eq!(target.load(\"k\").unwrap(), b\"v\");\n}}\n",
        consumer = spec.consumer,
        test = spec.test,
        p = spec.primary
    );
    let noise_src = "pub fn audit_trail(message: &str) {\n    println!(\"{message}\");\n}\n\npub fn render_summary(rows: &[Row]) -> String {\n    audit_trail(\"summary\");\n    rows.iter().map(|row| row.to_string()).collect()\n}\n\npub fn parse_flags(args: &[String]) -> Flags {\n    audit_trail(\"flags\");\n    Flags::from(args)\n}\n\npub fn schedule_cleanup() {\n    audit_trail(\"cleanup\");\n}\n";

    let trait_path = format!("src/{c_snake}.rs");
    let primary_path = format!("src/{}_{c_snake}.rs", snake(spec.primary));
    let mut builder = WorldBuilder::new()
        .file(&trait_path, &trait_src)
        .file(&primary_path, &primary_src)
        .file("src/consumer.rs", &consumer_src)
        .file("src/noise.rs", noise_src);

    builder = builder
        .symbol(
            "contract",
            &trait_path,
            c,
            SymbolKind::Trait,
            &format!("pub trait {c}"),
        )
        .symbol(
            "primary",
            &primary_path,
            &format!("{}{c}", spec.primary),
            SymbolKind::Struct,
            &format!("pub struct {}{c}", spec.primary),
        )
        .symbol(
            "seed",
            &primary_path,
            verb,
            SymbolKind::Method,
            &format!("fn {verb}("),
        )
        .symbol(
            "load",
            &primary_path,
            "load",
            SymbolKind::Method,
            "fn load(",
        )
        .symbol(
            "consumer",
            "src/consumer.rs",
            spec.consumer,
            SymbolKind::Function,
            &format!("pub fn {}", spec.consumer),
        )
        .test(
            "test",
            "src/consumer.rs",
            spec.test,
            &format!("fn {}", spec.test),
        )
        .symbol(
            "log",
            "src/noise.rs",
            "audit_trail",
            SymbolKind::Function,
            "pub fn audit_trail",
        )
        .symbol(
            "n0",
            "src/noise.rs",
            "render_summary",
            SymbolKind::Function,
            "pub fn render_summary",
        )
        .symbol(
            "n1",
            "src/noise.rs",
            "parse_flags",
            SymbolKind::Function,
            "pub fn parse_flags",
        )
        .symbol(
            "n2",
            "src/noise.rs",
            "schedule_cleanup",
            SymbolKind::Function,
            "pub fn schedule_cleanup",
        );
    let helper_keys: Vec<String> = (0..spec.helpers).map(|i| format!("h{i}")).collect();
    for (key, name) in helper_keys.iter().zip(&helper_names) {
        builder = builder.symbol(
            key,
            &primary_path,
            name,
            SymbolKind::Method,
            &format!("fn {name}("),
        );
    }
    let mut other_keys = Vec::new();
    for (at, other) in spec.others.iter().enumerate() {
        let path = format!("src/{}_{c_snake}.rs", snake(other));
        let src = format!(
            "pub struct {other}{c} {{\n    entries: Vec<u8>,\n}}\n\nimpl {c} for {other}{c} {{\n    fn {verb}(&self, key: &str, bytes: &[u8]) -> Result<(), Error> {{\n        self.entries.push(key, bytes);\n        Ok(())\n    }}\n\n    fn load(&self, key: &str) -> Result<Vec<u8>, Error> {{\n        self.entries.get(key)\n    }}\n}}\n"
        );
        let type_key = format!("o{at}");
        let method_key = format!("o{at}_{verb}");
        builder = builder
            .file(&path, &src)
            .symbol(
                &type_key,
                &path,
                &format!("{other}{c}"),
                SymbolKind::Struct,
                &format!("pub struct {other}{c}"),
            )
            .symbol(
                &method_key,
                &path,
                verb,
                SymbolKind::Method,
                &format!("fn {verb}("),
            );
        other_keys.push((type_key, method_key));
    }

    builder = builder
        .edge("primary", Inherits, "contract")
        .edge("primary", Contains, "seed")
        .edge("primary", Contains, "load")
        .edge("consumer", References, "primary")
        .edge("consumer", Calls, "seed")
        .edge("consumer", Calls, "log")
        .edge("test", References, "primary")
        .edge("test", Calls, "seed")
        .edge("n0", Calls, "log")
        .edge("n1", Calls, "log")
        .edge("n2", Calls, "log");
    for key in &helper_keys {
        builder = builder
            .edge("primary", Contains, key)
            .edge("seed", Calls, key);
    }
    for (type_key, method_key) in &other_keys {
        builder = builder
            .edge(type_key, Inherits, "contract")
            .edge(type_key, Contains, method_key);
    }

    let mut redundant: BTreeSet<String> = helper_keys.iter().cloned().collect();
    redundant.extend(other_keys.iter().map(|(_, method)| method.clone()));
    Case {
        world: builder.build(),
        intent: format!("{} {verb}", c.to_lowercase()),
        seed: "seed",
        relevant: ["contract", "primary", "consumer", "test"]
            .into_iter()
            .map(String::from)
            .collect(),
        redundant,
        noise: ["log", "n0", "n1", "n2"]
            .into_iter()
            .map(String::from)
            .collect(),
    }
}

/// How a way of selecting is configured.
struct Method {
    name: &'static str,
    /// `None` is the plain pack, with no adaptive selection.
    adaptive: Option<SelectionPolicy>,
}

fn hybrid(mode: Purpose) -> SelectionPolicy {
    SelectionPolicy::for_mode(mode)
}

fn idf_only() -> SelectionPolicy {
    let mut policy = hybrid(Purpose::Diagnose);
    policy.weights = Weights::LEXICAL_ONLY;
    policy.expansion = Expansion::Never;
    policy.diversity = 0.0;
    policy.role_bonus = 0.0;
    policy
}

fn ppr_only() -> SelectionPolicy {
    let mut policy = hybrid(Purpose::Diagnose);
    policy.weights = Weights::PPR_ONLY;
    policy.expansion = Expansion::Always;
    policy.diversity = 0.0;
    policy.role_bonus = 0.0;
    policy
}

fn without(
    mut policy: SelectionPolicy,
    tweak: impl FnOnce(&mut SelectionPolicy),
) -> SelectionPolicy {
    tweak(&mut policy);
    policy
}

fn methods() -> Vec<Method> {
    let base = hybrid(Purpose::Diagnose);
    vec![
        Method {
            name: "plain (legacy)",
            adaptive: None,
        },
        Method {
            name: "IDF only",
            adaptive: Some(idf_only()),
        },
        Method {
            name: "PPR only",
            adaptive: Some(ppr_only()),
        },
        Method {
            name: "hybrid diagnose",
            adaptive: Some(base.clone()),
        },
        Method {
            name: "hybrid refactor",
            adaptive: Some(hybrid(Purpose::Refactor)),
        },
        Method {
            name: "hybrid architecture",
            adaptive: Some(hybrid(Purpose::Architecture)),
        },
        Method {
            name: "- ppr",
            adaptive: Some(without(base.clone(), |p| p.weights.ppr = 0.0)),
        },
        Method {
            name: "- proximity",
            adaptive: Some(without(base.clone(), |p| p.weights.proximity = 0.0)),
        },
        Method {
            name: "- impact",
            adaptive: Some(without(base.clone(), |p| p.weights.impact = 0.0)),
        },
        Method {
            name: "- lexical",
            adaptive: Some(without(base.clone(), |p| p.weights.lexical = 0.0)),
        },
        Method {
            name: "- diversity",
            adaptive: Some(without(base.clone(), |p| p.diversity = 0.0)),
        },
        Method {
            name: "- role bonus",
            adaptive: Some(without(base, |p| p.role_bonus = 0.0)),
        },
    ]
}

/// The measure of one pack against the labels of its case.
#[derive(Debug, Default, Clone, Copy)]
struct Score {
    /// The share of the relevant set delivered.
    recall: f64,
    /// Items delivered that only repeat the seed.
    redundant: f64,
    /// Items delivered that are the hub or its callers.
    noise: f64,
    /// The share of the delivered items that are relevant.
    precision: f64,
    /// The cost of the text.
    used: f64,
}

/// How the seed is given.
#[derive(Clone, Copy, Debug)]
enum Request {
    /// The caller names the symbol.
    Anchored,
    /// Only the intent says what the request is about.
    Intent,
}

fn run(case: &Case, method: &Method, request: Request, budget: u64) -> Score {
    let adaptive = method.adaptive.clone().map(|policy| Adaptive {
        policy,
        budget: WorkBudget::default(),
    });
    let intent = Some(case.intent.as_str());
    let pack = match request {
        Request::Anchored => case.world.pack(&[case.seed], intent, adaptive, budget),
        Request::Intent => case.world.pack(&[], intent, adaptive, budget),
    };
    let mut relevant = case.relevant.clone();
    if matches!(request, Request::Intent) {
        relevant.insert(case.seed.to_string());
    }
    let keys = case.world.keys(&pack);
    let count = |set: &BTreeSet<String>| keys.iter().filter(|key| set.contains(*key)).count();
    let hits = count(&relevant);
    Score {
        recall: hits as f64 / relevant.len() as f64,
        redundant: count(&case.redundant) as f64,
        noise: count(&case.noise) as f64,
        precision: if keys.is_empty() {
            0.0
        } else {
            hits as f64 / keys.len() as f64
        },
        used: pack.budget.used.tokens as f64,
    }
}

/// The mean of the scores of a method over the whole corpus.
fn mean(cases: &[Case], method: &Method, request: Request, budget: u64) -> Score {
    let scores: Vec<Score> = cases
        .iter()
        .map(|case| run(case, method, request, budget))
        .collect();
    let n = scores.len() as f64;
    let avg = |pick: fn(&Score) -> f64| scores.iter().map(pick).sum::<f64>() / n;
    Score {
        recall: avg(|s| s.recall),
        redundant: avg(|s| s.redundant),
        noise: avg(|s| s.noise),
        precision: avg(|s| s.precision),
        used: avg(|s| s.used),
    }
}

fn corpus() -> Vec<Case> {
    SPECS.iter().map(build).collect()
}

/// The comparison, as a table.
fn table(cases: &[Case], request: Request, budget: u64) -> Vec<(&'static str, Score)> {
    methods()
        .iter()
        .map(|method| (method.name, mean(cases, method, request, budget)))
        .collect()
}

fn print(request: Request, budget: u64, rows: &[(&'static str, Score)]) {
    println!("== {request:?} @ {budget}");
    println!(
        "{:<22} {:>7} {:>9} {:>9} {:>6} {:>7}",
        "method", "recall", "precision", "redundant", "noise", "used"
    );
    for (name, s) in rows {
        println!(
            "{name:<22} {:>7.2} {:>9.2} {:>9.2} {:>6.2} {:>7.0}",
            s.recall, s.precision, s.redundant, s.noise, s.used
        );
    }
}

fn row<'a>(rows: &'a [(&'static str, Score)], name: &str) -> &'a Score {
    &rows
        .iter()
        .find(|(candidate, _)| *candidate == name)
        .unwrap_or_else(|| panic!("no method named {name}"))
        .1
}

/// The budget the claims are made at: tight enough that what goes in is a
/// choice, wide enough that the seed does not take all of it.
const CLAIM_BUDGET: u64 = 1500;

#[test]
fn the_hybrid_beats_the_name_alone_and_the_graph_alone_on_the_fixed_corpus() {
    let cases = corpus();
    for request in [Request::Anchored, Request::Intent] {
        let rows = table(&cases, request, CLAIM_BUDGET);
        print(request, CLAIM_BUDGET, &rows);
        let hybrid = row(&rows, "hybrid diagnose");
        let plain = row(&rows, "plain (legacy)");
        let idf = row(&rows, "IDF only");
        let ppr = row(&rows, "PPR only");

        assert!(
            hybrid.recall > idf.recall,
            "{request:?}: over the name alone"
        );
        assert!(
            hybrid.recall > ppr.recall,
            "{request:?}: over the graph alone"
        );
        assert!(
            hybrid.recall > plain.recall,
            "{request:?}: over the plain pack"
        );
        assert!(hybrid.precision > idf.precision.max(ppr.precision).max(plain.precision));
        assert!(
            hybrid.redundant < idf.redundant && hybrid.redundant < plain.redundant,
            "{request:?}: it repeats itself less"
        );
    }
}

#[test]
fn the_diversity_and_the_role_bonus_earn_their_place() {
    let cases = corpus();
    for request in [Request::Anchored, Request::Intent] {
        let rows = table(&cases, request, CLAIM_BUDGET);
        let hybrid = row(&rows, "hybrid diagnose");
        let no_diversity = row(&rows, "- diversity");
        let no_bonus = row(&rows, "- role bonus");

        assert!(
            no_diversity.precision < hybrid.precision || no_diversity.redundant > hybrid.redundant,
            "{request:?}: diversity improves precision or removes repetition"
        );
        assert!(
            no_bonus.recall < hybrid.recall,
            "{request:?}: the role bonus"
        );
    }
}

#[test]
fn the_graph_signals_each_help_somewhere_or_are_reported_as_not_helping() {
    let cases = corpus();
    let rows: Vec<_> = [Request::Anchored, Request::Intent]
        .into_iter()
        .map(|request| table(&cases, request, CLAIM_BUDGET))
        .collect();
    let mut verdicts = Vec::new();
    for ablation in ["- ppr", "- proximity", "- impact", "- lexical"] {
        let loses = rows.iter().any(|rows| {
            let hybrid = row(rows, "hybrid diagnose");
            let without = row(rows, ablation);
            without.recall < hybrid.recall || without.noise > hybrid.noise
        });
        verdicts.push((ablation, loses));
    }
    println!("signals that measurably help at {CLAIM_BUDGET}: {verdicts:?}");
    // The signals the evaluation keeps are the ones whose removal costs
    // recall or lets the hub in. `- proximity` costs neither on this corpus
    // at the defaults, and the policy documents it as unproven.
    for (ablation, loses) in &verdicts {
        if *ablation != "- proximity" {
            assert!(loses, "{ablation} changes nothing: discard the signal");
        }
    }
}

#[test]
fn the_evaluation_is_reproducible() {
    let cases = corpus();

    let first = table(&cases, Request::Anchored, CLAIM_BUDGET);
    let second = table(&cases, Request::Anchored, CLAIM_BUDGET);

    for ((name, a), (_, b)) in first.iter().zip(&second) {
        assert_eq!(
            (a.recall, a.precision, a.redundant, a.noise, a.used),
            (b.recall, b.precision, b.redundant, b.noise, b.used),
            "{name}"
        );
    }
}

#[test]
fn every_method_holds_the_budget() {
    let cases = corpus();
    for budget in [900, CLAIM_BUDGET, 2500] {
        for (name, score) in table(&cases, Request::Intent, budget) {
            assert!(score.used <= budget as f64, "{name} used {}", score.used);
        }
    }
}

#[test]
fn the_comparison_over_every_budget() {
    let cases = corpus();
    for request in [Request::Anchored, Request::Intent] {
        for budget in [900, 1500, 2500] {
            print(request, budget, &table(&cases, request, budget));
        }
    }
}

fn f1(score: &Score) -> f64 {
    if score.recall + score.precision == 0.0 {
        0.0
    } else {
        2.0 * score.recall * score.precision / (score.recall + score.precision)
    }
}

/// The mean F1 of a policy over some cases, both requests, two budgets.
fn objective(cases: &[Case], policy: &SelectionPolicy) -> (f64, Score) {
    let method = Method {
        name: "candidate",
        adaptive: Some(policy.clone()),
    };
    let mut total = 0.0;
    let mut sum = Score::default();
    let mut n = 0.0;
    for request in [Request::Anchored, Request::Intent] {
        for budget in [1500, 2500] {
            let s = mean(cases, &method, request, budget);
            total += f1(&s);
            sum.recall += s.recall;
            sum.precision += s.precision;
            sum.redundant += s.redundant;
            sum.noise += s.noise;
            n += 1.0;
        }
    }
    sum.recall /= n;
    sum.precision /= n;
    sum.redundant /= n;
    sum.noise /= n;
    (total / n, sum)
}

/// The values a search tries for each knob of a policy.
struct Grid<'a> {
    lexical: &'a [f64],
    ppr: &'a [f64],
    proximity: &'a [f64],
    impact: &'a [f64],
    role_bonus: &'a [f64],
    diversity: &'a [f64],
}

/// Scores every policy of the grid on the first three families and prints the
/// best ones with their score on the last two, which the search never sees.
fn search(grid: &Grid) {
    let all = corpus();
    let (train, held_out) = all.split_at(3);
    let mut rows: Vec<(f64, SelectionPolicy)> = Vec::new();
    for &lexical in grid.lexical {
        for &ppr in grid.ppr {
            for &proximity in grid.proximity {
                for &impact in grid.impact {
                    for &role_bonus in grid.role_bonus {
                        for &diversity in grid.diversity {
                            let mut policy = hybrid(Purpose::Diagnose);
                            policy.weights = Weights {
                                lexical,
                                ppr,
                                proximity,
                                impact,
                            };
                            policy.role_bonus = role_bonus;
                            policy.diversity = diversity;
                            rows.push((objective(train, &policy).0, policy));
                        }
                    }
                }
            }
        }
    }
    rows.sort_by(|a, b| b.0.total_cmp(&a.0));
    let default = hybrid(Purpose::Diagnose);
    println!(
        "default      train {:.3}  held-out {:.3}",
        objective(train, &default).0,
        objective(held_out, &default).0
    );
    for (value, policy) in rows.iter().take(12) {
        let w = policy.weights;
        println!(
            "lex {:.2} ppr {:.2} prox {:.2} imp {:.2} bonus {:.2} div {:.1}  train {value:.3}  held-out {:.3}",
            w.lexical,
            w.ppr,
            w.proximity,
            w.impact,
            policy.role_bonus,
            policy.diversity,
            objective(held_out, policy).0
        );
    }
}

/// The search the weights of the diagnose mode come from.
///
/// Run it with `cargo test --test it the_weights_search -- --ignored --nocapture`.
#[test]
#[ignore = "a search, not a check: it prints the evidence for the defaults"]
fn the_weights_search() {
    search(&Grid {
        lexical: &[0.0, 0.15, 0.3],
        ppr: &[0.0, 0.2, 0.4],
        proximity: &[0.2, 0.4, 0.6],
        impact: &[0.0, 0.1, 0.2],
        role_bonus: &[0.25, 0.5, 0.75],
        diversity: &[0.6, 0.8],
    });
}

/// The search of the knobs every mode shares, over the weights the first
/// search kept.
#[test]
#[ignore = "a search, not a check: it prints the evidence for the defaults"]
fn the_shared_knobs_search() {
    search(&Grid {
        lexical: &[0.15],
        ppr: &[0.2],
        proximity: &[0.2],
        impact: &[0.2],
        role_bonus: &[0.5, 0.75, 1.0, 1.5, 2.0],
        diversity: &[0.6, 0.8, 0.9, 1.0],
    });
}
