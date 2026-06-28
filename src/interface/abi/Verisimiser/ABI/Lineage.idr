-- SPDX-License-Identifier: MPL-2.0
-- Copyright (c) 2026 Jonathan D.A. Jewell (hyperpolymath) <j.d.a.jewell@open.ac.uk>
--
||| Lineage acyclicity for VeriSimiser.
|||
||| The Lineage octad dimension is a DAG: "what was derived from what" (ROADMAP
||| Phase 1, ADR-0005 — acyclic edges enforced via recursive CTE). This module
||| models lineage as edges over a topological index and proves the *defining*
||| invariant — the lineage graph is acyclic — as a genuine theorem, not a
||| runtime check:
|||
|||   1. A derivation edge always moves to a strictly greater topological index
|||      (`DerivedFrom`), so a derived node is never its own source.
|||   2. Any lineage *path* (transitive derivation) strictly increases the index
|||      (`lineageIncreases`).
|||   3. Therefore no node lies in its own lineage — there are no cycles
|||      (`noCycle`), and in particular no self-loops (`noSelfLoop`).
|||
||| The `LTE` helper lemmas are proved here from the constructors directly, so the
||| module depends on nothing that could hide an axiom.
module Verisimiser.ABI.Lineage

import Verisimiser.ABI.Types
import Data.Nat

%default total

--------------------------------------------------------------------------------
-- Order lemmas (proved constructively from LTE's constructors)
--------------------------------------------------------------------------------

||| Weakening on the right: `n <= m` implies `n <= S m`.
lteUp : LTE n m -> LTE n (S m)
lteUp LTEZero     = LTEZero
lteUp (LTESucc p) = LTESucc (lteUp p)

||| Strip a successor on the left: `S n <= m` implies `n <= m`.
lteDownLeft : LTE (S n) m -> LTE n m
lteDownLeft (LTESucc p) = lteUp p

||| Transitivity of `<=`.
lteChain : LTE a b -> LTE b c -> LTE a c
lteChain LTEZero     _           = LTEZero
lteChain (LTESucc p) (LTESucc q) = LTESucc (lteChain p q)

||| Transitivity of strict `<` (recall `LT a b = LTE (S a) b`).
ltChain : LT a b -> LT b c -> LT a c
ltChain ab bc = lteChain ab (lteDownLeft bc)

||| No natural is strictly less than itself — `S n <= n` is uninhabited.
ltIrreflexive : LT n n -> Void
ltIrreflexive (LTESucc p) = ltIrreflexive p

--------------------------------------------------------------------------------
-- Lineage model
--------------------------------------------------------------------------------

||| A single derivation edge: `dst` was derived from `src`. Well-formed exactly
||| when the derived node has a strictly greater topological index than its
||| source — this strict-decrease-toward-roots discipline is what makes the graph
||| a DAG.
public export
data DerivedFrom : (src, dst : Nat) -> Type where
  Derive : LT src dst -> DerivedFrom src dst

||| A lineage path: the transitive closure of derivation. `src` is (transitively)
||| an ancestor of `dst`.
public export
data Lineage : (src, dst : Nat) -> Type where
  ||| One derivation step.
  Direct : DerivedFrom src dst -> Lineage src dst
  ||| Compose a step with a longer tail.
  Then   : DerivedFrom src mid -> Lineage mid dst -> Lineage src dst

--------------------------------------------------------------------------------
-- Acyclicity theorems
--------------------------------------------------------------------------------

||| Every lineage path strictly increases the topological index.
export
lineageIncreases : Lineage src dst -> LT src dst
lineageIncreases (Direct (Derive lt))   = lt
lineageIncreases (Then (Derive lt) rest) = ltChain lt (lineageIncreases rest)

||| ACYCLICITY (the defining invariant): no node lies in its own lineage. If it
||| did, the path would force `LT n n`, which is impossible.
export
noCycle : Not (Lineage n n)
noCycle l = ltIrreflexive (lineageIncreases l)

||| In particular there are no self-loops: a node is never directly derived from
||| itself.
export
noSelfLoop : Not (DerivedFrom n n)
noSelfLoop (Derive lt) = ltIrreflexive lt

||| Non-vacuity: genuine derivations do exist (0 -> 1 is a well-formed edge), so
||| `DerivedFrom` is not the empty relation.
export
derivationsExist : DerivedFrom 0 1
derivationsExist = Derive (LTESucc LTEZero)

||| Non-vacuity: multi-hop lineage exists (0 -> 1 -> 2), so the acyclicity result
||| above is not vacuously true over an empty path space.
export
lineageExists : Lineage 0 2
lineageExists = Then (Derive (LTESucc LTEZero)) (Direct (Derive (LTESucc (LTESucc LTEZero))))
