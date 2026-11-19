//! Formal failure attribution (CA5): Halpern-Pearl actual causation on
//! binary structural models.
//!
//! "Which component actually caused the failure?" deserves a formal answer
//! on the trace, not a narrative. This module answers it with the
//! Halpern-Pearl definition of ACTUAL causation over a binary, bounded
//! structural causal model:
//!
//! - AC1: the query is stated over ACTUAL values - the attributed event is
//!   "the effect took its actual value", whatever that value is (querying
//!   an actually-false effect attributes the FALSE outcome; review P2-3).
//! - AC2(a): under some contingency (an assignment W = w'), flipping the
//!   cause flips the effect away from its actual value - a but-for test
//!   WITH contingencies.
//! - AC2(b): under the same contingency, with the cause at its actual value
//!   and the remaining variables (Z) frozen at their actual values, the
//!   effect still holds - the contingency must not silently break the path.
//!
//! Three boundaries, all deliberate:
//! 1. Contingency variables W draw from ENDOGENOUS variables only.
//!    Intervening on exogenous inputs would fabricate witnesses (in the
//!    canonical preemption example it would wrongly promote the second
//!    throw to a cause).
//! 2. Responsibility (Chockler & Halpern) is `1 / (1 + |W|)` over the
//!    SMALLEST witness - the search enumerates contingencies by ascending
//!    size, so the first witness found is the minimal one.
//! 3. The witness search stops at [`MAX_CONTINGENCY`]: a cause needing a
//!    larger minimal witness is reported as no-cause (documented
//!    incompleteness, not a refusal).
//!
//! Reference: Halpern & Pearl (2005); Halpern, "Actual Causality" (2016).

/// The upper bound on model size for the CYCLIC build path (actual-state
/// enumeration is exponential; refusing beats pretending).
pub const CYCLIC_BUILD_CAP: usize = 10;

/// The upper bound on endogenous variables for causal search (the
/// contingency enumeration contract).
pub const CAUSAL_SEARCH_CAP: usize = 12;

/// The upper bound on model size for the acyclic build path (a
/// topological evaluation is linear; this cap is a sanity contract).
pub const ACYCLIC_BUILD_CAP: usize = 32;

/// The largest contingency size the search enumerates. A cause whose
/// MINIMAL witness needs more variables than this cap is reported as
/// no-cause - a documented incompleteness, not a refusal: raising the cap
/// widens the search at combinatorial cost (review P1-2). High-arity
/// overdetermination beyond this cap needs a larger cap, not a fix.
pub const MAX_CONTINGENCY: usize = 4;

/// Build/verification refusals for attribution.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum AttributionError {
    /// A model with no variables has no causes.
    #[error("attribution refused: the model is empty")]
    EmptyModel,
    /// A parent or effect index outside the variable list.
    #[error("attribution refused: index {index} out of range for {vars} variables")]
    IndexOutOfRange {
        /// The offending index.
        index: usize,
        /// The variable count.
        vars: usize,
    },
    /// The equations have no fixed point, so the actual state is
    /// undefined.
    #[error("attribution refused: the equations have no consistent actual state")]
    NoConsistentActualState,
    /// The equations have several fixed points - the actual context is
    /// undetermined, and a cause of an undetermined fact is undefined.
    #[error(
        "attribution refused: the equations admit {solutions} actual states - the actual context must be unique"
    )]
    AmbiguousActualState {
        /// How many fixed points were found.
        solutions: usize,
    },
    /// A cyclic model above the enumeration contract.
    #[error("attribution refused: {vars} cyclic variables exceed the build contract of {cap}")]
    CyclicModelTooLarge {
        /// Requested variable count.
        vars: usize,
        /// The contract value ([`CYCLIC_BUILD_CAP`]).
        cap: usize,
    },
    /// A model above the acyclic sanity contract.
    #[error("attribution refused: {vars} variables exceed the acyclic build contract of {cap}")]
    AcyclicModelTooLarge {
        /// Requested variable count.
        vars: usize,
        /// The contract value ([`ACYCLIC_BUILD_CAP`]).
        cap: usize,
    },
    /// The effect index sits outside the variable list.
    #[error("attribution refused: effect index {index} out of range for {vars} variables")]
    EffectOutOfRange {
        /// The offending effect index.
        index: usize,
        /// The variable count.
        vars: usize,
    },
    /// Causal search over a model above the contingency contract.
    #[error(
        "attribution refused: {endogenous} endogenous variables exceed the search contract of {cap}"
    )]
    SearchTooLarge {
        /// The endogenous variable count.
        endogenous: usize,
        /// The contract value ([`CAUSAL_SEARCH_CAP`]).
        cap: usize,
    },
}

/// One structural equation: `value = func(state)`, reading ONLY the
/// declared parents from the full state vector. A variable with no parents
/// is exogenous: its function must return its (constant) actual value and
/// ignore the state.
pub type Equation = Box<dyn Fn(&[bool]) -> bool>;

/// One variable of the structural model.
pub struct VarDef {
    /// Variable name (for reports).
    pub name: String,
    /// Indices of the variables this equation reads.
    pub parents: Vec<usize>,
    /// The structural equation.
    pub func: Equation,
}

impl std::fmt::Debug for VarDef {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("VarDef")
            .field("name", &self.name)
            .field("parents", &self.parents)
            .finish_non_exhaustive()
    }
}

/// One found cause with its minimal witness.
#[derive(Debug, Clone, PartialEq)]
pub struct CauseReport {
    /// Index of the causing variable.
    pub cause: usize,
    /// Its name (copied from the model).
    pub cause_name: String,
    /// The minimal contingency: (variable index, intervened value) pairs.
    pub witness: Vec<(usize, bool)>,
    /// Chockler-Halpern responsibility `1 / (1 + |witness|)`: 1.0 for a
    /// pure but-for cause, halving with each additional contingency
    /// variable.
    pub responsibility: f64,
}

/// A binary structural causal model with a unique, verified actual state.
pub struct BinaryScm {
    defs: Vec<VarDef>,
    actual: Vec<bool>,
    acyclic: bool,
    /// Topological order over the dependency graph (acyclic models only).
    order: Vec<usize>,
}

impl std::fmt::Debug for BinaryScm {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("BinaryScm")
            .field(
                "names",
                &self.defs.iter().map(|d| &d.name).collect::<Vec<_>>(),
            )
            .field("actual", &self.actual)
            .finish()
    }
}

impl BinaryScm {
    /// Build and verify the model.
    ///
    /// Acyclicity is decided by DEPENDENCY GRAPH (Kahn's algorithm), not by
    /// index order - a DAG whose variables happen to be declared in reverse
    /// order is acyclic and takes the linear path. The actual state is
    /// computed (in topological order for acyclic models, by fixed-point
    /// enumeration for cyclic ones) and MUST be unique - a model whose
    /// equations admit zero or several actual states is refused, because a
    /// cause of an undetermined fact is undefined.
    pub fn build(defs: Vec<VarDef>) -> Result<Self, AttributionError> {
        if defs.is_empty() {
            return Err(AttributionError::EmptyModel);
        }
        let n = defs.len();
        for def in &defs {
            for &p in &def.parents {
                if p >= n {
                    return Err(AttributionError::IndexOutOfRange { index: p, vars: n });
                }
            }
        }
        // Kahn's algorithm over parent -> variable edges.
        let mut indegree: Vec<usize> = defs.iter().map(|d| d.parents.len()).collect();
        let mut children: Vec<Vec<usize>> = vec![Vec::new(); n];
        for (i, def) in defs.iter().enumerate() {
            for &p in &def.parents {
                children[p].push(i);
            }
        }
        let mut queue: std::collections::VecDeque<usize> =
            (0..n).filter(|&i| indegree[i] == 0).collect();
        let mut order = Vec::with_capacity(n);
        while let Some(i) = queue.pop_front() {
            order.push(i);
            for &child in &children[i] {
                indegree[child] -= 1;
                if indegree[child] == 0 {
                    queue.push_back(child);
                }
            }
        }
        let acyclic = order.len() == n;
        let actual = if acyclic {
            if n > ACYCLIC_BUILD_CAP {
                return Err(AttributionError::AcyclicModelTooLarge {
                    vars: n,
                    cap: ACYCLIC_BUILD_CAP,
                });
            }
            let mut state = vec![false; n];
            for &i in &order {
                state[i] = (defs[i].func)(&state);
            }
            state
        } else {
            if n > CYCLIC_BUILD_CAP {
                return Err(AttributionError::CyclicModelTooLarge {
                    vars: n,
                    cap: CYCLIC_BUILD_CAP,
                });
            }
            let mut solutions: Vec<Vec<bool>> = Vec::new();
            'assignment: for mask in 0u64..(1u64 << n) {
                let state: Vec<bool> = (0..n).map(|i| mask & (1u64 << i) != 0).collect();
                for (i, def) in defs.iter().enumerate() {
                    if (def.func)(&state) != state[i] {
                        continue 'assignment;
                    }
                }
                solutions.push(state);
                if solutions.len() > 1 {
                    return Err(AttributionError::AmbiguousActualState { solutions: 2 });
                }
            }
            match solutions.pop() {
                Some(state) => state,
                None => return Err(AttributionError::NoConsistentActualState),
            }
        };
        Ok(Self {
            defs,
            actual,
            acyclic,
            order,
        })
    }

    /// The verified actual state.
    #[must_use]
    pub fn actual(&self) -> &[bool] {
        &self.actual
    }

    /// Variable names in index order.
    #[must_use]
    pub fn names(&self) -> Vec<&str> {
        self.defs.iter().map(|d| d.name.as_str()).collect()
    }

    /// Endogenous variables (indices with at least one parent).
    fn endogenous(&self) -> Vec<usize> {
        self.defs
            .iter()
            .enumerate()
            .filter(|(_, d)| !d.parents.is_empty())
            .map(|(i, _)| i)
            .collect()
    }

    /// Evaluate an acyclic intervention: intervened values override,
    /// everything else recomputes in TOPOLOGICAL order (parents before
    /// dependents, regardless of declaration order).
    fn eval_acyclic(&self, partial: &[(usize, bool)]) -> Vec<bool> {
        let mut state = self.actual.clone();
        for &i in &self.order {
            if let Some((_, v)) = partial.iter().find(|(idx, _)| *idx == i) {
                state[i] = *v;
            } else {
                state[i] = (self.defs[i].func)(&state);
            }
        }
        state
    }

    /// Evaluate a cyclic intervention: enumerate fixed points over the
    /// free variables; `None` when the intervened system has no or several
    /// solutions (an ambiguous counterfactual must not manufacture a
    /// witness).
    fn eval_cyclic_unique(&self, partial: &[(usize, bool)]) -> Option<Vec<bool>> {
        let n = self.defs.len();
        let mut state = vec![false; n];
        for (i, v) in partial {
            state[*i] = *v;
        }
        let intervened: Vec<usize> = partial.iter().map(|(i, _)| *i).collect();
        let mut solutions = 0usize;
        let mut found = Vec::new();
        'assignment: for mask in 0u64..(1u64 << n) {
            for &i in &intervened {
                if (mask & (1u64 << i) != 0) != state[i] {
                    continue 'assignment; // disagrees with the intervention
                }
            }
            let candidate: Vec<bool> = (0..n).map(|i| mask & (1u64 << i) != 0).collect();
            for (i, def) in self.defs.iter().enumerate() {
                // An intervention REPLACES the variable's equation - the
                // original equation is not additionally required for
                // intervened variables (review P1-1: requiring it made
                // every flipped intervention unsatisfiable and the whole
                // cyclic attribution path dead).
                if intervened.contains(&i) {
                    continue;
                }
                if (def.func)(&candidate) != candidate[i] {
                    continue 'assignment;
                }
            }
            solutions += 1;
            if solutions > 1 {
                return None;
            }
            found = candidate;
        }
        if solutions == 1 { Some(found) } else { None }
    }

    /// AC2(a): with the cause flipped and the contingency set, the effect
    /// moves away from its actual value.
    fn ac2_a(
        &self,
        cause: usize,
        flipped: bool,
        contingency: &[(usize, bool)],
        effect: usize,
    ) -> bool {
        let mut partial: Vec<(usize, bool)> = vec![(cause, flipped)];
        partial.extend_from_slice(contingency);
        let state = if self.acyclic {
            self.eval_acyclic(&partial)
        } else {
            match self.eval_cyclic_unique(&partial) {
                Some(s) => s,
                None => return false, // ambiguous counterfactual: witness invalid
            }
        };
        state[effect] != self.actual[effect]
    }

    /// AC2(b): with the cause at its actual value and the contingency set,
    /// the remaining variables FROZEN at their actual values, the effect
    /// still holds. The effect's equation is evaluated with its parents
    /// drawn from: cause (actual value), contingency (intervened values),
    /// everything else (actual values).
    fn ac2_b(&self, cause: usize, contingency: &[(usize, bool)], effect: usize) -> bool {
        let effect_def = &self.defs[effect];
        let mut view = self.actual.clone();
        for &p in &effect_def.parents {
            let value = if p == cause {
                self.actual[cause]
            } else if let Some((_, v)) = contingency.iter().find(|(idx, _)| *idx == p) {
                *v
            } else {
                self.actual[p]
            };
            view[p] = value;
        }
        (effect_def.func)(&view) == self.actual[effect]
    }

    /// Find every actual cause of `effect` (Halpern-Pearl, single-variable
    /// causes) with minimal witnesses and responsibility scores.
    ///
    /// Semantics note (review P2-3): the query attributes the effect's
    /// ACTUAL value, whatever it is - for an actually-false effect the
    /// causes of the false outcome are returned. An empty result means
    /// "no cause found within the witness cap", never "provably no cause".
    pub fn causes_of(&self, effect: usize) -> Result<Vec<CauseReport>, AttributionError> {
        if effect >= self.defs.len() {
            return Err(AttributionError::EffectOutOfRange {
                index: effect,
                vars: self.defs.len(),
            });
        }
        let endogenous = self.endogenous();
        if endogenous.len() > CAUSAL_SEARCH_CAP {
            return Err(AttributionError::SearchTooLarge {
                endogenous: endogenous.len(),
                cap: CAUSAL_SEARCH_CAP,
            });
        }
        let mut reports = Vec::new();
        for cause in 0..self.defs.len() {
            if cause == effect {
                continue;
            }
            // Contingency candidates: endogenous, excluding cause and effect
            // (exogenous inputs are NEVER contingencies - see module note).
            let candidates: Vec<usize> = endogenous
                .iter()
                .copied()
                .filter(|&i| i != cause && i != effect)
                .collect();
            let mut found: Option<Vec<(usize, bool)>> = None;
            'sizes: for k in 0..=MAX_CONTINGENCY.min(candidates.len()) {
                for combo in combinations_of(&candidates, k) {
                    for mask in 0u64..(1u64 << k) {
                        let contingency: Vec<(usize, bool)> = combo
                            .iter()
                            .enumerate()
                            .map(|(j, &var)| (var, mask & (1u64 << j) != 0))
                            .collect();
                        let flipped = !self.actual[cause];
                        if self.ac2_a(cause, flipped, &contingency, effect)
                            && self.ac2_b(cause, &contingency, effect)
                        {
                            found = Some(contingency);
                            break 'sizes;
                        }
                    }
                }
            }
            if let Some(witness) = found {
                let responsibility = 1.0 / (1.0 + witness.len() as f64);
                reports.push(CauseReport {
                    cause,
                    cause_name: self.defs[cause].name.clone(),
                    witness,
                    responsibility,
                });
            }
        }
        Ok(reports)
    }
}

/// All k-subsets of `items` in lexicographic order (k <= items.len()).
fn combinations_of(items: &[usize], k: usize) -> Vec<Vec<usize>> {
    let mut result = Vec::new();
    if k == 0 {
        result.push(Vec::new());
        return result;
    }
    if k > items.len() {
        return result;
    }
    let mut combo: Vec<usize> = (0..k).collect();
    loop {
        result.push(combo.iter().map(|&i| items[i]).collect());
        let mut advanced = false;
        for idx in (0..k).rev() {
            if combo[idx] < items.len() - (k - idx) {
                combo[idx] += 1;
                for nxt in (idx + 1)..k {
                    combo[nxt] = combo[nxt - 1] + 1;
                }
                advanced = true;
                break;
            }
        }
        if !advanced {
            break;
        }
    }
    result
}
