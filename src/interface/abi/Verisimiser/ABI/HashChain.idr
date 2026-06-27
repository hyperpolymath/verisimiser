-- SPDX-License-Identifier: MPL-2.0
-- Copyright (c) 2026 Jonathan D.A. Jewell (hyperpolymath) <j.d.a.jewell@open.ac.uk>
--
||| Provenance hash-chain integrity for VeriSimiser.
|||
||| The Provenance octad dimension is a hash chain (ROADMAP Phase 1 — a write-path
||| observer with a SHA-256 hash chain): every entry records the hash of its
||| predecessor, so tampering with an earlier entry breaks every hash downstream.
||| This module models that chain and proves its integrity properties:
|||
|||   1. Integrity by construction — `ProvChain` can only be extended by an entry
|||      that links onto the chain's *current* tip, so a `ProvChain t` value is a
|||      proof that every link is intact.
|||   2. The runtime replay verifier accepts a correctly-linked entry
|||      (`replayOne`) and rejects a forged predecessor link (`replayReject`).
|||   3. No link can masquerade as the genesis record (`linkHashNeverGenesis`),
|||      and changing the hashed content changes the tip (`contentChangesTip`) —
|||      so the chain genuinely binds its contents.
module Verisimiser.ABI.HashChain

import Verisimiser.ABI.Types
import Data.Nat

%default total

||| Integer tag for the operation recorded in a provenance entry.
public export
opTag : ProvenanceOperation -> Nat
opTag Create    = 0
opTag Update    = 1
opTag Delete    = 2
opTag Transform = 3

||| The tip hash of an empty provenance log.
public export
genesisHash : Nat
genesisHash = 0

||| Deterministic modelled digest of a new link: folds the predecessor tip with
||| the operation tag and the content digest. The leading `S` makes every link
||| hash non-zero, so no link can be confused with `genesisHash`.
public export
linkHash : (prev : Nat) -> (op : ProvenanceOperation) -> (content : Nat) -> Nat
linkHash prev op content = S (prev + opTag op + content)

||| A provenance hash chain indexed by its current tip hash.
|||
||| `Append` forces the new entry to chain onto the chain's *actual* current tip,
||| and the resulting tip is `linkHash` of that entry. A value of `ProvChain t` is
||| therefore a proof that every link is intact: integrity by construction.
public export
data ProvChain : (tip : Nat) -> Type where
  ||| The empty log: its tip is `genesisHash` (= 0). Written as the literal `0`
  ||| because a bare lowercase `genesisHash` in an index position would be
  ||| auto-bound as a fresh implicit (inhabiting every tip and silently voiding
  ||| the integrity guarantee).
  Origin : ProvChain 0
  Append : (op : ProvenanceOperation) -> (content : Nat) ->
           ProvChain prev -> ProvChain (linkHash prev op content)

||| A stored provenance entry as it appears on disk / over the FFI: the operation,
||| the content digest, and the predecessor hash the writer *recorded*.
public export
record StoredLink where
  constructor MkLink
  op      : ProvenanceOperation
  content : Nat
  recPrev : Nat

||| Runtime verifier: replay stored links from a starting tip, checking that each
||| entry's recorded predecessor hash matches the running tip. Returns the final
||| tip if intact, `Nothing` at the first broken link.
public export
replay : (tip : Nat) -> List StoredLink -> Maybe Nat
replay tip []                            = Just tip
replay tip (MkLink op content recPrev :: rest) =
  if recPrev == tip
    then replay (linkHash tip op content) rest
    else Nothing

--------------------------------------------------------------------------------
-- Verifier soundness lemmas (general, not just concrete witnesses)
--------------------------------------------------------------------------------

||| `==` on `Nat` is reflexive — needed to discharge the "predecessor matches the
||| running tip" branch of `replay` for an arbitrary tip.
eqNatRefl : (n : Nat) -> (n == n) = True
eqNatRefl 0     = Refl
eqNatRefl (S k) = eqNatRefl k

||| The verifier ACCEPTS a correctly-linked entry: when the recorded predecessor
||| equals the current tip, replay advances the tip by `linkHash`.
export
replayOne : (tip : Nat) -> (op : ProvenanceOperation) -> (content : Nat) ->
            replay tip [MkLink op content tip] = Just (linkHash tip op content)
replayOne tip op content = rewrite eqNatRefl tip in Refl

||| The verifier REJECTS a forged link: when the recorded predecessor differs from
||| the current tip, replay fails at that entry. (Non-vacuous tamper detection.)
export
replayReject : (tip, bad : Nat) -> (op : ProvenanceOperation) -> (content : Nat) ->
               (bad == tip) = False ->
               replay tip [MkLink op content bad] = Nothing
replayReject tip bad op content noteq = rewrite noteq in Refl

--------------------------------------------------------------------------------
-- Anti-forgery properties of the hash itself
--------------------------------------------------------------------------------

||| No link hash can equal the genesis hash, so a forged entry can never pose as
||| the start of the chain.
export
linkHashNeverGenesis : (prev : Nat) -> (op : ProvenanceOperation) -> (content : Nat) ->
                       Not (linkHash prev op content = 0)
linkHashNeverGenesis prev op content Refl impossible

||| The tip binds the hashed content: distinct content under the same predecessor
||| and operation yields a distinct tip (a concrete, non-vacuous witness that the
||| chain is not blind to its payload).
export
contentChangesTip : Not (linkHash 0 Create 1 = linkHash 0 Create 2)
contentChangesTip Refl impossible

--------------------------------------------------------------------------------
-- Concrete end-to-end controls
--------------------------------------------------------------------------------

||| A correctly-linked two-entry chain replays from genesis to its computed tip.
||| (genesis = 0; first tip = linkHash 0 Create 10 = S (0+0+10) = 11;
|||  second tip = linkHash 11 Update 5 = S (11+1+5) = 18.)
export
intactChainReplays :
  replay 0 [MkLink Create 10 0, MkLink Update 5 11] = Just 18
intactChainReplays = Refl

||| Tampering with the second entry's recorded predecessor (12 instead of 11)
||| makes replay fail — the end-to-end negative control.
export
tamperedChainFails :
  replay 0 [MkLink Create 10 0, MkLink Update 5 12] = Nothing
tamperedChainFails = Refl
