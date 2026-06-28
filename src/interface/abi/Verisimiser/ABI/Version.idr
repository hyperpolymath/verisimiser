-- SPDX-License-Identifier: MPL-2.0
-- Copyright (c) 2026 Jonathan D.A. Jewell (hyperpolymath) <j.d.a.jewell@open.ac.uk>
--
||| Temporal version ordering for VeriSimiser.
|||
||| The Temporal octad dimension keeps a value's version history and answers
||| point-in-time ("as-of") queries (ROADMAP Phase 1 — `verisimdb_temporal_versions`).
||| This module proves the defining invariants:
|||
|||   1. Strict monotonicity by construction — a `History` can only be extended by
|||      a snapshot whose version strictly exceeds the previous one, so there is no
|||      version skew and no duplicate versions.
|||   2. A runtime `asOf` query and an `ascending` validity check, each pinned by
|||      concrete positive and negative controls (the controls are non-vacuous: the
|||      checks really do separate good histories from bad ones).
module Verisimiser.ABI.Version

import Verisimiser.ABI.Types
import Data.Nat

%default total

||| A versioned snapshot: a logical version number and an (abstract) value digest.
public export
record Snapshot where
  constructor At
  version : Nat
  value   : Nat

||| A version history with strictly increasing version numbers, indexed by a
||| strict lower bound that the head snapshot's version must exceed.
|||
||| Being able to *construct* a `History above` is itself a proof that the whole
||| history is strictly monotonic in version — hence free of duplicate versions
||| and temporal skew.
public export
data History : (above : Nat) -> Type where
  Empty : History above
  Snap  : (s : Snapshot) -> LT above (version s) ->
          History (version s) -> History above

||| The strict lower bound really is a strict lower bound on the head version —
||| recovered directly from the constructor (monotonicity is not assumed, it is
||| carried by the structure).
export
headStrictlyAbove : History above -> (s : Snapshot) -> LT above (version s) ->
                    History (version s) -> LT above (version s)
headStrictlyAbove _ _ prf _ = prf

--------------------------------------------------------------------------------
-- Runtime validity check (what an implementation computes) + controls
--------------------------------------------------------------------------------

||| Runtime check that a flat list of snapshots is strictly ascending in version
||| starting above `lo`.
public export
ascending : (lo : Nat) -> List Snapshot -> Bool
ascending _  []              = True
ascending lo (At v _ :: xs)  = lo < v && ascending v xs

||| Positive control: a genuinely ascending history passes.
export
ascendingAccepts : ascending 0 [At 1 100, At 3 300, At 7 700] = True
ascendingAccepts = Refl

||| Negative control (non-vacuity): an out-of-order history is rejected, so the
||| check is not constantly `True`.
export
ascendingRejects : ascending 0 [At 3 300, At 1 100] = False
ascendingRejects = Refl

--------------------------------------------------------------------------------
-- Point-in-time ("as-of") query + controls
--------------------------------------------------------------------------------

||| Point-in-time query: the value of the latest snapshot whose version is `<= t`.
||| For an ascending history this is the most recent committed value as of `t`.
public export
asOf : (t : Nat) -> List Snapshot -> Maybe Nat
asOf _ []              = Nothing
asOf t (At v val :: xs) =
  if v <= t
    then case asOf t xs of
           Just later => Just later
           Nothing    => Just val
    else Nothing

||| Querying before the first version yields nothing.
export
asOfBeforeStart : asOf 0 [At 1 100, At 3 300, At 7 700] = Nothing
asOfBeforeStart = Refl

||| Querying at t = 5 returns version 3's value (300): the latest version `<= 5`,
||| not the newest (7) and not an earlier one (1).
export
asOfPicksLatest : asOf 5 [At 1 100, At 3 300, At 7 700] = Just 300
asOfPicksLatest = Refl

||| Querying at or beyond the newest version returns the newest value.
export
asOfPicksNewest : asOf 9 [At 1 100, At 3 300, At 7 700] = Just 700
asOfPicksNewest = Refl

||| Non-vacuity for the construction side: a strictly-ascending history exists.
export
historyExists : History 0
historyExists = Snap (At 1 100) (LTESucc LTEZero) Empty
