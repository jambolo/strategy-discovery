//! Tiered feature vocabulary and the composable feature-expression algebra.
//!
//! Features form an open, tiered vocabulary: tier 1 (primitive) features fall out
//! mechanically from game structure, tier 2 (supplied) features are hand-authored
//! per game, and tier 3 (invented) features are expressions over lower-tier
//! features. Tier 1/2 features are computed natively by game-side code; tier 3
//! features carry a [`FeatureExpr`] that this module evaluates.

use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use std::fmt;
use thiserror::Error;

/// Feature tier: where a feature's value comes from.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Tier {
    /// Derived mechanically from game structure (e.g. from a symmetry group).
    Primitive,
    /// Hand-supplied per-game strategic feature.
    Supplied,
    /// Invented at runtime as an expression over lower-tier features.
    Invented,
}

impl fmt::Display for Tier {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let s = match self {
            Tier::Primitive => "tier-1",
            Tier::Supplied => "tier-2",
            Tier::Invented => "tier-3",
        };
        write!(f, "{s}")
    }
}

/// Runtime value of a feature.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", content = "value", rename_all = "kebab-case")]
pub enum FeatureValue {
    /// A boolean feature value.
    Bool(bool),
    /// An integer feature value.
    Int(i64),
    /// A floating-point feature value.
    Float(f64),
    /// A set of position indices.
    Set(BTreeSet<usize>),
}

impl FeatureValue {
    /// The type name of this value: `"bool"`, `"int"`, `"float"`, or `"set"`.
    pub fn type_name(&self) -> &'static str {
        match self {
            FeatureValue::Bool(_) => "bool",
            FeatureValue::Int(_) => "int",
            FeatureValue::Float(_) => "float",
            FeatureValue::Set(_) => "set",
        }
    }

    /// Extracts the boolean value, or a `TypeMismatch` if this is not a `Bool`.
    pub fn as_bool(&self) -> Result<bool, FeatureError> {
        match self {
            FeatureValue::Bool(b) => Ok(*b),
            other => Err(FeatureError::TypeMismatch {
                expected: "bool",
                actual: other.type_name(),
            }),
        }
    }

    /// Extracts the integer value, or a `TypeMismatch` if this is not an `Int`.
    pub fn as_int(&self) -> Result<i64, FeatureError> {
        match self {
            FeatureValue::Int(i) => Ok(*i),
            other => Err(FeatureError::TypeMismatch {
                expected: "int",
                actual: other.type_name(),
            }),
        }
    }

    /// Extracts a floating-point value. `Int` coerces to `f64`; `Float` returns itself.
    pub fn as_float(&self) -> Result<f64, FeatureError> {
        match self {
            FeatureValue::Int(i) => Ok(*i as f64),
            FeatureValue::Float(x) => Ok(*x),
            other => Err(FeatureError::TypeMismatch {
                expected: "float",
                actual: other.type_name(),
            }),
        }
    }

    /// Extracts the set value, or a `TypeMismatch` if this is not a `Set`.
    pub fn as_set(&self) -> Result<&BTreeSet<usize>, FeatureError> {
        match self {
            FeatureValue::Set(s) => Ok(s),
            other => Err(FeatureError::TypeMismatch {
                expected: "set",
                actual: other.type_name(),
            }),
        }
    }
}

impl fmt::Display for FeatureValue {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            FeatureValue::Bool(b) => write!(f, "{b}"),
            FeatureValue::Int(i) => write!(f, "{i}"),
            FeatureValue::Float(x) => write!(f, "{x}"),
            FeatureValue::Set(s) => {
                write!(f, "{{")?;
                for (i, v) in s.iter().enumerate() {
                    if i > 0 {
                        write!(f, ", ")?;
                    }
                    write!(f, "{v}")?;
                }
                write!(f, "}}")
            }
        }
    }
}

/// Comparison operator for [`FeatureExpr::Cmp`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum CmpOp {
    /// Equal.
    Eq,
    /// Not equal.
    Ne,
    /// Less than (numeric only).
    Lt,
    /// Less than or equal (numeric only).
    Le,
    /// Greater than (numeric only).
    Gt,
    /// Greater than or equal (numeric only).
    Ge,
}

impl fmt::Display for CmpOp {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let s = match self {
            CmpOp::Eq => "==",
            CmpOp::Ne => "!=",
            CmpOp::Lt => "<",
            CmpOp::Le => "<=",
            CmpOp::Gt => ">",
            CmpOp::Ge => ">=",
        };
        write!(f, "{s}")
    }
}

/// Arithmetic operator for [`FeatureExpr::Arith`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ArithOp {
    /// Addition.
    Add,
    /// Subtraction.
    Sub,
    /// Multiplication.
    Mul,
}

impl fmt::Display for ArithOp {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let s = match self {
            ArithOp::Add => "+",
            ArithOp::Sub => "-",
            ArithOp::Mul => "*",
        };
        write!(f, "{s}")
    }
}

/// Set operator for [`FeatureExpr::SetOp`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum SetOp {
    /// Set union.
    Union,
    /// Set intersection.
    Intersection,
    /// Set difference (left minus right).
    Difference,
}

impl fmt::Display for SetOp {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let s = match self {
            SetOp::Union => "|",
            SetOp::Intersection => "&",
            SetOp::Difference => "\\",
        };
        write!(f, "{s}")
    }
}

/// Feature-expression algebra: constants, references, boolean logic,
/// comparisons, arithmetic, and set operations over position indices.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "op", rename_all = "kebab-case")]
pub enum FeatureExpr {
    /// A literal value.
    Const {
        /// The literal value.
        value: FeatureValue,
    },
    /// A reference to a named feature, resolved via a [`FeatureEnv`] at evaluation time.
    Ref {
        /// The referenced feature's name.
        name: String,
    },
    /// Boolean negation.
    Not {
        /// The operand, which must evaluate to `Bool`.
        expr: Box<FeatureExpr>,
    },
    /// Boolean conjunction over zero or more operands. Empty is `true`.
    And {
        /// Operands, each of which must evaluate to `Bool`.
        exprs: Vec<FeatureExpr>,
    },
    /// Boolean disjunction over zero or more operands. Empty is `false`.
    Or {
        /// Operands, each of which must evaluate to `Bool`.
        exprs: Vec<FeatureExpr>,
    },
    /// A comparison between two values.
    Cmp {
        /// The comparison operator.
        ///
        /// Serialized as `operator` (not `op`) to avoid colliding with this
        /// enum's `#[serde(tag = "op")]` discriminant.
        #[serde(rename = "operator")]
        op: CmpOp,
        /// Left-hand operand.
        lhs: Box<FeatureExpr>,
        /// Right-hand operand.
        rhs: Box<FeatureExpr>,
    },
    /// An arithmetic operation between two numeric values.
    Arith {
        /// The arithmetic operator.
        ///
        /// Serialized as `operator` (not `op`) to avoid colliding with this
        /// enum's `#[serde(tag = "op")]` discriminant.
        #[serde(rename = "operator")]
        op: ArithOp,
        /// Left-hand operand.
        lhs: Box<FeatureExpr>,
        /// Right-hand operand.
        rhs: Box<FeatureExpr>,
    },
    /// The cardinality of a set, as an `Int`.
    Count {
        /// The operand, which must evaluate to `Set`.
        expr: Box<FeatureExpr>,
    },
    /// A set operation between two sets.
    SetOp {
        /// The set operator.
        ///
        /// Serialized as `operator` (not `op`) to avoid colliding with this
        /// enum's `#[serde(tag = "op")]` discriminant.
        #[serde(rename = "operator")]
        op: SetOp,
        /// Left-hand operand.
        lhs: Box<FeatureExpr>,
        /// Right-hand operand.
        rhs: Box<FeatureExpr>,
    },
    /// Whether `element` (an `Int`) is a member of `set` (a `Set`).
    Contains {
        /// The set operand.
        set: Box<FeatureExpr>,
        /// The element operand.
        element: Box<FeatureExpr>,
    },
}

impl FeatureExpr {
    /// A reference to a named feature.
    pub fn named(name: impl Into<String>) -> Self {
        FeatureExpr::Ref { name: name.into() }
    }

    /// An integer constant.
    pub fn int(value: i64) -> Self {
        FeatureExpr::Const {
            value: FeatureValue::Int(value),
        }
    }

    /// A boolean constant.
    pub fn boolean(value: bool) -> Self {
        FeatureExpr::Const {
            value: FeatureValue::Bool(value),
        }
    }

    /// A floating-point constant.
    pub fn float(value: f64) -> Self {
        FeatureExpr::Const {
            value: FeatureValue::Float(value),
        }
    }

    /// A set constant built from position indices.
    pub fn set(positions: impl IntoIterator<Item = usize>) -> Self {
        FeatureExpr::Const {
            value: FeatureValue::Set(positions.into_iter().collect()),
        }
    }

    /// Boolean negation of `expr`.
    pub fn negate(expr: FeatureExpr) -> Self {
        FeatureExpr::Not { expr: Box::new(expr) }
    }

    /// Conjunction of `exprs`.
    pub fn all(exprs: Vec<FeatureExpr>) -> Self {
        FeatureExpr::And { exprs }
    }

    /// Disjunction of `exprs`.
    pub fn any(exprs: Vec<FeatureExpr>) -> Self {
        FeatureExpr::Or { exprs }
    }

    /// A comparison `lhs op rhs`.
    pub fn compare(op: CmpOp, lhs: FeatureExpr, rhs: FeatureExpr) -> Self {
        FeatureExpr::Cmp {
            op,
            lhs: Box::new(lhs),
            rhs: Box::new(rhs),
        }
    }

    /// An arithmetic operation `lhs op rhs`.
    pub fn arith(op: ArithOp, lhs: FeatureExpr, rhs: FeatureExpr) -> Self {
        FeatureExpr::Arith {
            op,
            lhs: Box::new(lhs),
            rhs: Box::new(rhs),
        }
    }

    /// The cardinality of `expr`.
    pub fn count(expr: FeatureExpr) -> Self {
        FeatureExpr::Count { expr: Box::new(expr) }
    }

    /// A set operation `lhs op rhs`.
    pub fn set_op(op: SetOp, lhs: FeatureExpr, rhs: FeatureExpr) -> Self {
        FeatureExpr::SetOp {
            op,
            lhs: Box::new(lhs),
            rhs: Box::new(rhs),
        }
    }

    /// Whether `element` is a member of `set`.
    pub fn contains(set: FeatureExpr, element: FeatureExpr) -> Self {
        FeatureExpr::Contains {
            set: Box::new(set),
            element: Box::new(element),
        }
    }

    /// Every `Ref` name appearing in this expression tree, sorted and deduplicated.
    pub fn references(&self) -> BTreeSet<String> {
        let mut out = BTreeSet::new();
        self.collect_references(&mut out);
        out
    }

    fn collect_references(&self, out: &mut BTreeSet<String>) {
        match self {
            FeatureExpr::Const { .. } => {}
            FeatureExpr::Ref { name } => {
                out.insert(name.clone());
            }
            FeatureExpr::Not { expr } | FeatureExpr::Count { expr } => {
                expr.collect_references(out);
            }
            FeatureExpr::And { exprs } | FeatureExpr::Or { exprs } => {
                for e in exprs {
                    e.collect_references(out);
                }
            }
            FeatureExpr::Cmp { lhs, rhs, .. } | FeatureExpr::Arith { lhs, rhs, .. } | FeatureExpr::SetOp { lhs, rhs, .. } => {
                lhs.collect_references(out);
                rhs.collect_references(out);
            }
            FeatureExpr::Contains { set, element } => {
                set.collect_references(out);
                element.collect_references(out);
            }
        }
    }

    /// Evaluates this expression against `env`, resolving every `Ref` via lookup.
    pub fn evaluate(&self, env: &dyn FeatureEnv) -> Result<FeatureValue, FeatureError> {
        match self {
            FeatureExpr::Const { value } => Ok(value.clone()),
            FeatureExpr::Ref { name } => env.lookup(name).ok_or_else(|| FeatureError::Unknown(name.clone())),
            FeatureExpr::Not { expr } => {
                let b = expr.evaluate(env)?.as_bool()?;
                Ok(FeatureValue::Bool(!b))
            }
            FeatureExpr::And { exprs } => {
                let mut result = true;
                for e in exprs {
                    let b = e.evaluate(env)?.as_bool()?;
                    result &= b;
                }
                Ok(FeatureValue::Bool(result))
            }
            FeatureExpr::Or { exprs } => {
                let mut result = false;
                for e in exprs {
                    let b = e.evaluate(env)?.as_bool()?;
                    result |= b;
                }
                Ok(FeatureValue::Bool(result))
            }
            FeatureExpr::Cmp { op, lhs, rhs } => {
                let l = lhs.evaluate(env)?;
                let r = rhs.evaluate(env)?;
                evaluate_cmp(*op, l, r)
            }
            FeatureExpr::Arith { op, lhs, rhs } => {
                let l = lhs.evaluate(env)?;
                let r = rhs.evaluate(env)?;
                evaluate_arith(*op, l, r)
            }
            FeatureExpr::Count { expr } => {
                let v = expr.evaluate(env)?;
                let s = v.as_set()?;
                Ok(FeatureValue::Int(s.len() as i64))
            }
            FeatureExpr::SetOp { op, lhs, rhs } => {
                let l = lhs.evaluate(env)?;
                let r = rhs.evaluate(env)?;
                let a = l.as_set()?;
                let b = r.as_set()?;
                let result: BTreeSet<usize> = match op {
                    SetOp::Union => a.union(b).copied().collect(),
                    SetOp::Intersection => a.intersection(b).copied().collect(),
                    SetOp::Difference => a.difference(b).copied().collect(),
                };
                Ok(FeatureValue::Set(result))
            }
            FeatureExpr::Contains { set, element } => {
                let s = set.evaluate(env)?;
                let set_val = s.as_set()?;
                let e = element.evaluate(env)?.as_int()?;
                if e < 0 {
                    return Ok(FeatureValue::Bool(false));
                }
                Ok(FeatureValue::Bool(set_val.contains(&(e as usize))))
            }
        }
    }
}

fn evaluate_cmp(op: CmpOp, l: FeatureValue, r: FeatureValue) -> Result<FeatureValue, FeatureError> {
    match (&l, &r) {
        (FeatureValue::Bool(a), FeatureValue::Bool(b)) => match op {
            CmpOp::Eq => Ok(FeatureValue::Bool(a == b)),
            CmpOp::Ne => Ok(FeatureValue::Bool(a != b)),
            _ => Err(FeatureError::TypeMismatch {
                expected: "int",
                actual: "bool",
            }),
        },
        (FeatureValue::Set(a), FeatureValue::Set(b)) => match op {
            CmpOp::Eq => Ok(FeatureValue::Bool(a == b)),
            CmpOp::Ne => Ok(FeatureValue::Bool(a != b)),
            _ => Err(FeatureError::TypeMismatch {
                expected: "int",
                actual: "set",
            }),
        },
        (FeatureValue::Int(_) | FeatureValue::Float(_), FeatureValue::Int(_) | FeatureValue::Float(_)) => {
            let lf = l.as_float()?;
            let rf = r.as_float()?;
            let b = match op {
                CmpOp::Eq => lf == rf,
                CmpOp::Ne => lf != rf,
                CmpOp::Lt => lf < rf,
                CmpOp::Le => lf <= rf,
                CmpOp::Gt => lf > rf,
                CmpOp::Ge => lf >= rf,
            };
            Ok(FeatureValue::Bool(b))
        }
        _ => Err(FeatureError::TypeMismatch {
            expected: l.type_name(),
            actual: r.type_name(),
        }),
    }
}

fn evaluate_arith(op: ArithOp, l: FeatureValue, r: FeatureValue) -> Result<FeatureValue, FeatureError> {
    match (&l, &r) {
        (FeatureValue::Int(a), FeatureValue::Int(b)) => {
            let res = match op {
                ArithOp::Add => a.checked_add(*b),
                ArithOp::Sub => a.checked_sub(*b),
                ArithOp::Mul => a.checked_mul(*b),
            };
            res.map(FeatureValue::Int).ok_or(FeatureError::Overflow)
        }
        (FeatureValue::Int(_) | FeatureValue::Float(_), FeatureValue::Int(_) | FeatureValue::Float(_)) => {
            let lf = l.as_float()?;
            let rf = r.as_float()?;
            let res = match op {
                ArithOp::Add => lf + rf,
                ArithOp::Sub => lf - rf,
                ArithOp::Mul => lf * rf,
            };
            Ok(FeatureValue::Float(res))
        }
        _ => {
            let bad = if matches!(l, FeatureValue::Int(_) | FeatureValue::Float(_)) {
                &r
            } else {
                &l
            };
            Err(FeatureError::TypeMismatch {
                expected: "int",
                actual: bad.type_name(),
            })
        }
    }
}

impl fmt::Display for FeatureExpr {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            FeatureExpr::Const { value } => write!(f, "{value}"),
            FeatureExpr::Ref { name } => write!(f, "{name}"),
            FeatureExpr::Not { expr } => write!(f, "(not {expr})"),
            FeatureExpr::And { exprs } => {
                write!(f, "(")?;
                for (i, e) in exprs.iter().enumerate() {
                    if i > 0 {
                        write!(f, " and ")?;
                    }
                    write!(f, "{e}")?;
                }
                write!(f, ")")
            }
            FeatureExpr::Or { exprs } => {
                write!(f, "(")?;
                for (i, e) in exprs.iter().enumerate() {
                    if i > 0 {
                        write!(f, " or ")?;
                    }
                    write!(f, "{e}")?;
                }
                write!(f, ")")
            }
            FeatureExpr::Cmp { op, lhs, rhs } => write!(f, "({lhs} {op} {rhs})"),
            FeatureExpr::Arith { op, lhs, rhs } => write!(f, "({lhs} {op} {rhs})"),
            FeatureExpr::Count { expr } => write!(f, "count({expr})"),
            FeatureExpr::SetOp { op, lhs, rhs } => write!(f, "({lhs} {op} {rhs})"),
            FeatureExpr::Contains { set, element } => write!(f, "({element} in {set})"),
        }
    }
}

/// Name-to-value resolver used during [`FeatureExpr::evaluate`].
pub trait FeatureEnv {
    /// Looks up the current value of a named feature, if known.
    fn lookup(&self, name: &str) -> Option<FeatureValue>;
}

/// Named feature values for one state.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct FeatureVector(pub BTreeMap<String, FeatureValue>);

impl FeatureVector {
    /// An empty feature vector.
    pub fn new() -> Self {
        Self::default()
    }

    /// Inserts or overwrites the value of a named feature.
    pub fn insert(&mut self, name: impl Into<String>, value: FeatureValue) {
        self.0.insert(name.into(), value);
    }

    /// Looks up the value of a named feature.
    pub fn get(&self, name: &str) -> Option<&FeatureValue> {
        self.0.get(name)
    }

    /// The number of features held.
    pub fn len(&self) -> usize {
        self.0.len()
    }

    /// Whether this vector holds no features.
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    /// Iterates over `(name, value)` pairs in name order.
    pub fn iter(&self) -> impl Iterator<Item = (&String, &FeatureValue)> {
        self.0.iter()
    }

    /// Returns `self` merged with `other`; on duplicate names, `other` wins.
    pub fn merged(&self, other: &FeatureVector) -> FeatureVector {
        let mut out = self.0.clone();
        for (name, value) in other.0.iter() {
            out.insert(name.clone(), value.clone());
        }
        FeatureVector(out)
    }
}

impl FeatureEnv for FeatureVector {
    fn lookup(&self, name: &str) -> Option<FeatureValue> {
        self.0.get(name).cloned()
    }
}

/// Errors that can arise while validating or evaluating features.
#[derive(Debug, Error, Clone, PartialEq)]
pub enum FeatureError {
    /// Referenced a feature name that has no known value or definition.
    #[error("unknown feature `{0}`")]
    Unknown(String),
    /// Attempted to define a feature name that already exists.
    #[error("duplicate feature `{0}`")]
    Duplicate(String),
    /// A value did not have the type an operation required.
    #[error("type mismatch: expected {expected}, got {actual}")]
    TypeMismatch {
        /// The type name the operation required.
        expected: &'static str,
        /// The type name of the value actually supplied.
        actual: &'static str,
    },
    /// An integer arithmetic operation overflowed `i64`.
    #[error("integer overflow")]
    Overflow,
}

/// A named feature and how it is obtained.
///
/// `expr: None` means the feature is native: computed by game-side code.
/// `expr: Some(_)` means the feature is derived from lower-tier names.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FeatureDef {
    /// The feature's unique name.
    pub name: String,
    /// The feature's tier.
    pub tier: Tier,
    /// A human-readable description.
    pub description: String,
    /// The derivation expression, or `None` if this feature is native.
    pub expr: Option<FeatureExpr>,
}

impl FeatureDef {
    /// Defines a native feature, computed by game-side code.
    pub fn native(name: impl Into<String>, tier: Tier, description: impl Into<String>) -> Self {
        FeatureDef {
            name: name.into(),
            tier,
            description: description.into(),
            expr: None,
        }
    }

    /// Defines a derived feature, computed from `expr`.
    pub fn derived(name: impl Into<String>, tier: Tier, description: impl Into<String>, expr: FeatureExpr) -> Self {
        FeatureDef {
            name: name.into(),
            tier,
            description: description.into(),
            expr: Some(expr),
        }
    }

    /// Whether this feature is native (has no derivation expression).
    pub fn is_native(&self) -> bool {
        self.expr.is_none()
    }
}

impl fmt::Display for FeatureDef {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match &self.expr {
            Some(expr) => write!(f, "{} [{}] = {} -- {}", self.name, self.tier, expr, self.description),
            None => write!(f, "{} [{}] = <native> -- {}", self.name, self.tier, self.description),
        }
    }
}

/// An ordered, self-consistent list of feature definitions.
///
/// A derived definition may reference only names defined earlier in the list
/// (native or derived alike).
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct FeatureVocabulary {
    defs: Vec<FeatureDef>,
}

impl FeatureVocabulary {
    /// An empty vocabulary.
    pub fn new() -> Self {
        Self::default()
    }

    /// Appends `def`, rejecting a duplicate name (`Duplicate`) or a derived
    /// definition referencing a name not already defined (`Unknown`).
    pub fn push(&mut self, def: FeatureDef) -> Result<(), FeatureError> {
        if self.contains(&def.name) {
            return Err(FeatureError::Duplicate(def.name));
        }
        if let Some(expr) = &def.expr {
            for name in expr.references() {
                if !self.contains(&name) {
                    return Err(FeatureError::Unknown(name));
                }
            }
        }
        self.defs.push(def);
        Ok(())
    }

    /// Re-runs the [`Self::push`] checks over the whole list; use after deserialization.
    pub fn validate(&self) -> Result<(), FeatureError> {
        let mut rebuilt = FeatureVocabulary::new();
        for def in &self.defs {
            rebuilt.push(def.clone())?;
        }
        Ok(())
    }

    /// Looks up a definition by name.
    pub fn get(&self, name: &str) -> Option<&FeatureDef> {
        self.defs.iter().find(|d| d.name == name)
    }

    /// Whether a definition with this name exists.
    pub fn contains(&self, name: &str) -> bool {
        self.get(name).is_some()
    }

    /// All definitions, in vocabulary order.
    pub fn defs(&self) -> &[FeatureDef] {
        &self.defs
    }

    /// The number of definitions held.
    pub fn len(&self) -> usize {
        self.defs.len()
    }

    /// Whether this vocabulary holds no definitions.
    pub fn is_empty(&self) -> bool {
        self.defs.is_empty()
    }

    /// Evaluates every derived definition in list order on top of `native`;
    /// returns `native` union derived values.
    pub fn evaluate(&self, native: &FeatureVector) -> Result<FeatureVector, FeatureError> {
        let mut acc = native.clone();
        for def in &self.defs {
            if let Some(expr) = &def.expr {
                let value = expr.evaluate(&acc)?;
                acc.insert(def.name.clone(), value);
            }
        }
        Ok(acc)
    }

    /// Definitions for `names` plus everything they transitively reference,
    /// in vocabulary order. Names absent from the vocabulary are ignored.
    pub fn closure(&self, names: &BTreeSet<String>) -> Vec<FeatureDef> {
        let mut needed: BTreeSet<String> = BTreeSet::new();
        let mut stack: Vec<String> = names.iter().filter(|n| self.contains(n)).cloned().collect();
        while let Some(name) = stack.pop() {
            if !needed.insert(name.clone()) {
                continue;
            }
            if let Some(def) = self.get(&name)
                && let Some(expr) = &def.expr
            {
                for r in expr.references() {
                    if self.contains(&r) && !needed.contains(&r) {
                        stack.push(r);
                    }
                }
            }
        }
        self.defs.iter().filter(|d| needed.contains(&d.name)).cloned().collect()
    }
}

impl fmt::Display for FeatureVocabulary {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let lines: Vec<String> = self.defs.iter().map(|d| d.to_string()).collect();
        write!(f, "{}", lines.join("\n"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct MapEnv(BTreeMap<String, FeatureValue>);
    impl FeatureEnv for MapEnv {
        fn lookup(&self, name: &str) -> Option<FeatureValue> {
            self.0.get(name).cloned()
        }
    }

    fn map_env(pairs: Vec<(&str, FeatureValue)>) -> MapEnv {
        MapEnv(pairs.into_iter().map(|(k, v)| (k.to_string(), v)).collect())
    }

    // --- serde round-trips ---

    #[test]
    fn feature_value_roundtrip() {
        let values = vec![
            FeatureValue::Bool(true),
            FeatureValue::Int(-7),
            FeatureValue::Float(2.5),
            FeatureValue::Set(BTreeSet::from([0, 2, 6])),
        ];
        for v in values {
            let s = serde_json::to_string(&v).unwrap();
            let back: FeatureValue = serde_json::from_str(&s).unwrap();
            assert_eq!(v, back);
        }
    }

    #[test]
    fn feature_value_int_json_shape() {
        assert_eq!(
            serde_json::to_string(&FeatureValue::Int(3)).unwrap(),
            r#"{"type":"int","value":3}"#
        );
    }

    #[test]
    fn feature_expr_ref_json_shape() {
        assert_eq!(
            serde_json::to_string(&FeatureExpr::named("a")).unwrap(),
            r#"{"op":"ref","name":"a"}"#
        );
    }

    #[test]
    fn feature_expr_roundtrip_all_variants() {
        let inner = FeatureExpr::contains(
            FeatureExpr::set_op(SetOp::Union, FeatureExpr::set([0, 1]), FeatureExpr::set([2])),
            FeatureExpr::arith(ArithOp::Add, FeatureExpr::int(1), FeatureExpr::int(1)),
        );
        let expr = FeatureExpr::all(vec![
            FeatureExpr::any(vec![
                FeatureExpr::negate(FeatureExpr::boolean(false)),
                FeatureExpr::compare(CmpOp::Ge, FeatureExpr::float(1.5), FeatureExpr::int(1)),
            ]),
            FeatureExpr::compare(CmpOp::Eq, FeatureExpr::count(FeatureExpr::named("wins")), FeatureExpr::int(0)),
            inner,
        ]);
        let s = serde_json::to_string(&expr).unwrap();
        let back: FeatureExpr = serde_json::from_str(&s).unwrap();
        assert_eq!(expr, back);
    }

    #[test]
    fn feature_def_roundtrip() {
        let native = FeatureDef::native("wins", Tier::Supplied, "winning positions");
        let derived = FeatureDef::derived(
            "has_win",
            Tier::Invented,
            "a winning move exists",
            FeatureExpr::compare(CmpOp::Gt, FeatureExpr::count(FeatureExpr::named("wins")), FeatureExpr::int(0)),
        );
        for def in [native, derived] {
            let s = serde_json::to_string(&def).unwrap();
            let back: FeatureDef = serde_json::from_str(&s).unwrap();
            assert_eq!(def, back);
        }
    }

    #[test]
    fn feature_vocabulary_roundtrip() {
        let mut vocab = FeatureVocabulary::new();
        vocab.push(FeatureDef::native("a", Tier::Primitive, "native a")).unwrap();
        vocab
            .push(FeatureDef::derived(
                "b",
                Tier::Invented,
                "derived b",
                FeatureExpr::arith(ArithOp::Add, FeatureExpr::named("a"), FeatureExpr::int(1)),
            ))
            .unwrap();
        let s = serde_json::to_string(&vocab).unwrap();
        let back: FeatureVocabulary = serde_json::from_str(&s).unwrap();
        assert_eq!(vocab, back);
    }

    // --- Display ---

    #[test]
    fn display_nested_expr() {
        let expr = FeatureExpr::all(vec![
            FeatureExpr::compare(CmpOp::Gt, FeatureExpr::count(FeatureExpr::named("wins")), FeatureExpr::int(0)),
            FeatureExpr::negate(FeatureExpr::named("blocked")),
        ]);
        assert_eq!(expr.to_string(), "((count(wins) > 0) and (not blocked))");
    }

    #[test]
    fn display_set_value() {
        let v = FeatureValue::Set(BTreeSet::from([0, 2, 6]));
        assert_eq!(v.to_string(), "{0, 2, 6}");
    }

    #[test]
    fn display_empty_set_value() {
        let v = FeatureValue::Set(BTreeSet::new());
        assert_eq!(v.to_string(), "{}");
    }

    #[test]
    fn display_derived_def() {
        let def = FeatureDef::derived(
            "has_win",
            Tier::Invented,
            "a winning move exists",
            FeatureExpr::compare(CmpOp::Gt, FeatureExpr::count(FeatureExpr::named("wins")), FeatureExpr::int(0)),
        );
        assert_eq!(
            def.to_string(),
            "has_win [tier-3] = (count(wins) > 0) -- a winning move exists"
        );
    }

    #[test]
    fn display_native_def() {
        let def = FeatureDef::native("wins", Tier::Supplied, "winning positions");
        assert_eq!(def.to_string(), "wins [tier-2] = <native> -- winning positions");
    }

    // --- Evaluate ---

    #[test]
    fn evaluate_and_true() {
        let env = map_env(vec![
            ("wins", FeatureValue::Set(BTreeSet::from([0, 4]))),
            ("blocked", FeatureValue::Bool(false)),
        ]);
        let expr = FeatureExpr::all(vec![
            FeatureExpr::compare(CmpOp::Gt, FeatureExpr::count(FeatureExpr::named("wins")), FeatureExpr::int(0)),
            FeatureExpr::negate(FeatureExpr::named("blocked")),
        ]);
        assert_eq!(expr.evaluate(&env).unwrap(), FeatureValue::Bool(true));
    }

    #[test]
    fn evaluate_and_type_mismatch() {
        let env = map_env(vec![]);
        let expr = FeatureExpr::all(vec![FeatureExpr::int(1)]);
        assert_eq!(
            expr.evaluate(&env),
            Err(FeatureError::TypeMismatch {
                expected: "bool",
                actual: "int",
            })
        );
    }

    #[test]
    fn evaluate_unknown_ref() {
        let env = map_env(vec![]);
        let expr = FeatureExpr::named("nope");
        assert_eq!(expr.evaluate(&env), Err(FeatureError::Unknown("nope".to_string())));
    }

    #[test]
    fn evaluate_arith_int_float_coerces_to_float() {
        let env = map_env(vec![]);
        let expr = FeatureExpr::arith(ArithOp::Add, FeatureExpr::int(1), FeatureExpr::float(0.5));
        assert_eq!(expr.evaluate(&env).unwrap(), FeatureValue::Float(1.5));
    }

    #[test]
    fn evaluate_arith_overflow() {
        let env = map_env(vec![]);
        let expr = FeatureExpr::arith(ArithOp::Add, FeatureExpr::int(i64::MAX), FeatureExpr::int(1));
        assert_eq!(expr.evaluate(&env), Err(FeatureError::Overflow));
    }

    #[test]
    fn evaluate_cmp_int_float_eq() {
        let env = map_env(vec![]);
        let expr = FeatureExpr::compare(CmpOp::Eq, FeatureExpr::int(2), FeatureExpr::float(2.0));
        assert_eq!(expr.evaluate(&env).unwrap(), FeatureValue::Bool(true));
    }

    #[test]
    fn evaluate_contains_true() {
        let env = map_env(vec![]);
        let expr = FeatureExpr::contains(FeatureExpr::set([1, 3]), FeatureExpr::int(3));
        assert_eq!(expr.evaluate(&env).unwrap(), FeatureValue::Bool(true));
    }

    #[test]
    fn evaluate_set_difference() {
        let env = map_env(vec![]);
        let expr = FeatureExpr::set_op(SetOp::Difference, FeatureExpr::set([0, 1, 2]), FeatureExpr::set([1]));
        assert_eq!(expr.evaluate(&env).unwrap(), FeatureValue::Set(BTreeSet::from([0, 2])));
    }

    #[test]
    fn evaluate_count_empty_set() {
        let env = map_env(vec![]);
        let expr = FeatureExpr::count(FeatureExpr::set([]));
        assert_eq!(expr.evaluate(&env).unwrap(), FeatureValue::Int(0));
    }

    // --- Vocabulary ---

    #[test]
    fn vocabulary_evaluate_chains_derived() {
        let mut vocab = FeatureVocabulary::new();
        vocab.push(FeatureDef::native("a", Tier::Primitive, "native a")).unwrap();
        vocab
            .push(FeatureDef::derived(
                "b",
                Tier::Invented,
                "derived b",
                FeatureExpr::arith(ArithOp::Add, FeatureExpr::named("a"), FeatureExpr::int(1)),
            ))
            .unwrap();

        let mut native = FeatureVector::new();
        native.insert("a", FeatureValue::Int(2));
        let result = vocab.evaluate(&native).unwrap();
        assert_eq!(result.get("a"), Some(&FeatureValue::Int(2)));
        assert_eq!(result.get("b"), Some(&FeatureValue::Int(3)));
    }

    #[test]
    fn vocabulary_push_unknown_ref() {
        let mut vocab = FeatureVocabulary::new();
        vocab.push(FeatureDef::native("a", Tier::Primitive, "native a")).unwrap();
        let result = vocab.push(FeatureDef::derived(
            "c",
            Tier::Invented,
            "derived c",
            FeatureExpr::named("zzz"),
        ));
        assert_eq!(result, Err(FeatureError::Unknown("zzz".to_string())));
    }

    #[test]
    fn vocabulary_push_duplicate() {
        let mut vocab = FeatureVocabulary::new();
        vocab.push(FeatureDef::native("a", Tier::Primitive, "native a")).unwrap();
        let result = vocab.push(FeatureDef::native("a", Tier::Primitive, "native a again"));
        assert_eq!(result, Err(FeatureError::Duplicate("a".to_string())));
    }

    #[test]
    fn vocabulary_closure() {
        let mut vocab = FeatureVocabulary::new();
        vocab.push(FeatureDef::native("a", Tier::Primitive, "native a")).unwrap();
        vocab
            .push(FeatureDef::derived(
                "b",
                Tier::Invented,
                "derived b",
                FeatureExpr::arith(ArithOp::Add, FeatureExpr::named("a"), FeatureExpr::int(1)),
            ))
            .unwrap();
        let closure = vocab.closure(&BTreeSet::from(["b".to_string()]));
        let names: Vec<&str> = closure.iter().map(|d| d.name.as_str()).collect();
        assert_eq!(names, vec!["a", "b"]);
    }

    #[test]
    fn vocabulary_validate_after_roundtrip() {
        let mut vocab = FeatureVocabulary::new();
        vocab.push(FeatureDef::native("a", Tier::Primitive, "native a")).unwrap();
        vocab
            .push(FeatureDef::derived(
                "b",
                Tier::Invented,
                "derived b",
                FeatureExpr::arith(ArithOp::Add, FeatureExpr::named("a"), FeatureExpr::int(1)),
            ))
            .unwrap();
        let s = serde_json::to_string(&vocab).unwrap();
        let back: FeatureVocabulary = serde_json::from_str(&s).unwrap();
        assert_eq!(back.validate(), Ok(()));
    }
}
