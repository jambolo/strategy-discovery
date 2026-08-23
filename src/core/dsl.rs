//! Heuristic-strategy DSL: an ordered decision list of `condition -> action`
//! rules with priorities, over the [`crate::core::features`] algebra.
//!
//! The framework owns rule *forms* here; games supply the feature *vocabulary*
//! the conditions and selectors reference. A [`HeuristicStrategy`] is
//! self-contained: every feature name it references must be defined in its own
//! `definitions`, so an emitted heuristic over invented concepts stays
//! readable without external context. Execution (turning a strategy into
//! moves) is a later phase; this module defines types, ordering, validation,
//! and a fixed pretty-print.

use crate::core::features::{FeatureDef, FeatureError, FeatureExpr, FeatureVocabulary};
use crate::core::kinds::HEURISTIC_RULES;
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;
use std::fmt;
use thiserror::Error;

/// How a fired rule picks among legal actions.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "select", rename_all = "kebab-case")]
pub enum ActionSelector {
    /// Any legal action (interpreter tie-breaks with its seeded RNG).
    AnyLegal,
    /// A legal action whose target position is in the set-valued `expr`.
    TargetIn {
        /// A set-valued expression naming the acceptable target positions.
        expr: FeatureExpr,
    },
    /// The legal action whose resulting state maximizes numeric `expr` (one-ply lookahead).
    Maximize {
        /// A numeric-valued expression evaluated after each candidate move.
        expr: FeatureExpr,
    },
}

impl ActionSelector {
    /// Feature names referenced by the selector (empty for `AnyLegal`).
    pub fn references(&self) -> BTreeSet<String> {
        match self {
            ActionSelector::AnyLegal => BTreeSet::new(),
            ActionSelector::TargetIn { expr } | ActionSelector::Maximize { expr } => expr.references(),
        }
    }
}

impl fmt::Display for ActionSelector {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ActionSelector::AnyLegal => write!(f, "any legal action"),
            ActionSelector::TargetIn { expr } => write!(f, "play a position in {expr}"),
            ActionSelector::Maximize { expr } => write!(f, "maximize {expr}"),
        }
    }
}

/// One entry in a [`HeuristicStrategy`]'s decision list.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Rule {
    /// A short, human-readable rule name.
    pub name: String,
    /// Firing order: higher priority is tried first; ties keep list order.
    pub priority: i32,
    /// Must evaluate to `FeatureValue::Bool`.
    pub condition: FeatureExpr,
    /// How to pick an action once this rule fires.
    pub action: ActionSelector,
}

impl Rule {
    /// Builds a new rule.
    pub fn new(name: impl Into<String>, priority: i32, condition: FeatureExpr, action: ActionSelector) -> Self {
        Rule {
            name: name.into(),
            priority,
            condition,
            action,
        }
    }

    /// Feature names referenced by this rule's condition or action.
    pub fn references(&self) -> BTreeSet<String> {
        let mut out = self.condition.references();
        out.extend(self.action.references());
        out
    }
}

impl fmt::Display for Rule {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "[{}] {}: if {} then {}",
            self.priority, self.name, self.condition, self.action
        )
    }
}

/// Errors from validating a [`HeuristicStrategy`].
#[derive(Debug, Error, Clone, PartialEq)]
pub enum DslError {
    /// The strategy's `kind` field is not `"heuristic-rules"`.
    #[error("strategy kind `{0}` is not `heuristic-rules`")]
    WrongKind(String),
    /// A rule or the fallback references a feature name absent from `definitions`.
    #[error("undefined features: {0:?}")]
    UndefinedFeatures(Vec<String>),
    /// A native definition names a feature the game's featurizer does not produce.
    #[error("native features not produced by this game: {0:?}")]
    UnavailableFeatures(Vec<String>),
    /// The feature vocabulary itself failed validation.
    #[error(transparent)]
    Feature(#[from] FeatureError),
}

/// Ordered decision list over features — the discovered-strategy representation.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct HeuristicStrategy {
    /// A human-readable strategy name.
    pub name: String,
    /// Always `"heuristic-rules"`; stored so persisted strategies self-identify their kind.
    pub kind: String,
    /// Self-contained definitions for every feature this strategy references.
    pub definitions: FeatureVocabulary,
    /// The decision list, in authoring order (see [`HeuristicStrategy::ordered_rules`] for firing order).
    pub rules: Vec<Rule>,
    /// The action selector used when no rule fires.
    pub fallback: ActionSelector,
}

impl HeuristicStrategy {
    /// A new strategy: `kind = HEURISTIC_RULES`, empty definitions, no rules, `fallback = AnyLegal`.
    pub fn new(name: impl Into<String>) -> Self {
        HeuristicStrategy {
            name: name.into(),
            kind: HEURISTIC_RULES.to_string(),
            definitions: FeatureVocabulary::new(),
            rules: Vec::new(),
            fallback: ActionSelector::AnyLegal,
        }
    }

    /// Adds a feature definition. Delegates to [`FeatureVocabulary::push`].
    pub fn define(&mut self, def: FeatureDef) -> Result<(), FeatureError> {
        self.definitions.push(def)
    }

    /// Appends a rule to the decision list.
    pub fn push_rule(&mut self, rule: Rule) {
        self.rules.push(rule);
    }

    /// Rules sorted by `priority` descending, stable (ties keep list order).
    pub fn ordered_rules(&self) -> Vec<&Rule> {
        let mut ordered: Vec<&Rule> = self.rules.iter().collect();
        ordered.sort_by(|a, b| b.priority.cmp(&a.priority));
        ordered
    }

    /// Every feature name referenced by any rule or the fallback.
    pub fn references(&self) -> BTreeSet<String> {
        let mut out = BTreeSet::new();
        for rule in &self.rules {
            out.extend(rule.references());
        }
        out.extend(self.fallback.references());
        out
    }

    /// Referenced names missing from `definitions`, sorted.
    pub fn undefined_references(&self) -> Vec<String> {
        self.references()
            .into_iter()
            .filter(|name| !self.definitions.contains(name))
            .collect()
    }

    /// Validates this strategy: `kind == HEURISTIC_RULES` (else `WrongKind`),
    /// `definitions.validate()`, then `undefined_references()` empty (else
    /// `UndefinedFeatures`), checked in that order.
    pub fn validate(&self) -> Result<(), DslError> {
        if self.kind != HEURISTIC_RULES {
            return Err(DslError::WrongKind(self.kind.clone()));
        }
        self.definitions.validate()?;
        let undefined = self.undefined_references();
        if !undefined.is_empty() {
            return Err(DslError::UndefinedFeatures(undefined));
        }
        Ok(())
    }
}

impl fmt::Display for HeuristicStrategy {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        writeln!(f, "strategy {} ({})", self.name, self.kind)?;
        if self.definitions.is_empty() {
            writeln!(f, "features: (none)")?;
        } else {
            writeln!(f, "features:")?;
            for def in self.definitions.defs() {
                writeln!(f, "  {def}")?;
            }
        }
        if self.rules.is_empty() {
            writeln!(f, "rules: (none)")?;
        } else {
            writeln!(f, "rules:")?;
            for (i, rule) in self.ordered_rules().into_iter().enumerate() {
                if i > 0 {
                    writeln!(f)?;
                }
                write!(f, "  {rule}")?;
            }
            writeln!(f)?;
        }
        write!(f, "otherwise: {}", self.fallback)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::features::{CmpOp, FeatureExpr, Tier};

    fn demo() -> HeuristicStrategy {
        let mut strategy = HeuristicStrategy::new("demo");
        strategy
            .define(FeatureDef::native(
                "wins",
                Tier::Supplied,
                "positions completing a line for the player to move",
            ))
            .unwrap();
        strategy
            .define(FeatureDef::derived(
                "has_win",
                Tier::Invented,
                "a winning move exists",
                FeatureExpr::compare(CmpOp::Gt, FeatureExpr::count(FeatureExpr::named("wins")), FeatureExpr::int(0)),
            ))
            .unwrap();
        strategy.push_rule(Rule::new(
            "take-win",
            100,
            FeatureExpr::named("has_win"),
            ActionSelector::TargetIn {
                expr: FeatureExpr::named("wins"),
            },
        ));
        strategy.push_rule(Rule::new("anything", 0, FeatureExpr::boolean(true), ActionSelector::AnyLegal));
        strategy
    }

    #[test]
    fn demo_display() {
        let expected = concat!(
            "strategy demo (heuristic-rules)\n",
            "features:\n",
            "  wins [tier-2] = <native> -- positions completing a line for the player to move\n",
            "  has_win [tier-3] = (count(wins) > 0) -- a winning move exists\n",
            "rules:\n",
            "  [100] take-win: if has_win then play a position in wins\n",
            "  [0] anything: if true then any legal action\n",
            "otherwise: any legal action"
        );
        assert_eq!(demo().to_string(), expected);
    }

    #[test]
    fn empty_display() {
        let expected = "strategy empty (heuristic-rules)\nfeatures: (none)\nrules: (none)\notherwise: any legal action";
        assert_eq!(HeuristicStrategy::new("empty").to_string(), expected);
    }

    #[test]
    fn ordering_stable_ties() {
        let mut strategy = HeuristicStrategy::new("order");
        strategy.push_rule(Rule::new("a", 1, FeatureExpr::boolean(true), ActionSelector::AnyLegal));
        strategy.push_rule(Rule::new("b", 5, FeatureExpr::boolean(true), ActionSelector::AnyLegal));
        strategy.push_rule(Rule::new("c", 5, FeatureExpr::boolean(true), ActionSelector::AnyLegal));
        strategy.push_rule(Rule::new("d", 3, FeatureExpr::boolean(true), ActionSelector::AnyLegal));
        let names: Vec<&str> = strategy.ordered_rules().into_iter().map(|r| r.name.as_str()).collect();
        assert_eq!(names, vec!["b", "c", "d", "a"]);
    }

    #[test]
    fn demo_validates() {
        assert_eq!(demo().validate(), Ok(()));
    }

    #[test]
    fn demo_references() {
        let expected: BTreeSet<String> = ["has_win", "wins"].into_iter().map(String::from).collect();
        assert_eq!(demo().references(), expected);
    }

    #[test]
    fn undefined_reference_fails_validation() {
        let mut strategy = HeuristicStrategy::new("ghosted");
        strategy.push_rule(Rule::new("spooky", 0, FeatureExpr::named("ghost"), ActionSelector::AnyLegal));
        assert_eq!(
            strategy.validate(),
            Err(DslError::UndefinedFeatures(vec!["ghost".to_string()]))
        );
    }

    #[test]
    fn wrong_kind_checked_first() {
        let mut strategy = HeuristicStrategy::new("ghosted");
        strategy.kind = "minimax".to_string();
        strategy.push_rule(Rule::new("spooky", 0, FeatureExpr::named("ghost"), ActionSelector::AnyLegal));
        assert_eq!(strategy.validate(), Err(DslError::WrongKind("minimax".to_string())));
    }

    #[test]
    fn demo_json_roundtrip() {
        let strategy = demo();
        let json = serde_json::to_string(&strategy).unwrap();
        assert!(json.contains(r#""kind":"heuristic-rules""#));
        assert!(json.contains(r#""select":"target-in""#));
        let back: HeuristicStrategy = serde_json::from_str(&json).unwrap();
        assert_eq!(back, strategy);
    }
}
