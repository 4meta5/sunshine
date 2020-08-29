#![recursion_limit = "256"]
//! # Court Module
//! This module expresses a framework for dispute resolution. It stores vote metadata
//! to schedule and dispatch votes to resolve disputes when they arise.
//!
//! - [`court::Trait`](./trait.Trait.html)
//! - [`Call`](./enum.Call.html)
//!
//! ## Overview
//!
//! This pallet introduces the notion of counterparty insurance, with accountability
//! enforced by the outcome of an org vote, dispatched when/if the dispute
//! arises.
//!
//! [`Call`]: ./enum.Call.html
//! [`Trait`]: ./trait.Trait.html
#![cfg_attr(not(feature = "std"), no_std)]

#[cfg(test)]
mod tests;

use codec::Codec;
use frame_support::{
    decl_error,
    decl_event,
    decl_module,
    decl_storage,
    ensure,
    traits::{
        Currency,
        ExistenceRequirement,
        Get,
        ReservableCurrency,
    },
    Parameter,
};
use frame_system::{
    ensure_signed,
    Trait as System,
};
use org::Trait as Org;
use sp_runtime::{
    traits::{
        AtLeast32Bit,
        MaybeSerializeDeserialize,
        Member,
        Zero,
    },
    DispatchError,
    DispatchResult,
    Permill,
};
use sp_std::{
    fmt::Debug,
    prelude::*,
};
use util::{
    court::{
        Dispute,
        DisputeState,
    },
    meta::VoteMetadata,
    organization::OrgRep,
    traits::{
        GenerateUniqueID,
        GetVoteOutcome,
        IDIsAvailable,
        OpenVote,
        RegisterDisputeType,
    },
    vote::VoteOutcome,
};
use vote::Trait as Vote;

/// The balances type for this module
type BalanceOf<T> =
    <<T as Trait>::Currency as Currency<<T as System>::AccountId>>::Balance;
type GovernanceOf<T> = VoteMetadata<
    OrgRep<<T as Org>::OrgId>,
    <T as Vote>::Signal,
    Permill,
    <T as System>::BlockNumber,
>;
type DisputeOf<T> = Dispute<
    <T as System>::AccountId,
    BalanceOf<T>,
    <T as System>::BlockNumber,
    GovernanceOf<T>,
    DisputeState<<T as Vote>::VoteId>,
>;
pub trait Trait: System + Org + Vote {
    /// The overarching event type
    type Event: From<Event<Self>> + Into<<Self as System>::Event>;

    /// The currency type
    type Currency: Currency<Self::AccountId>
        + ReservableCurrency<Self::AccountId>;

    /// The identifier for disputes
    type DisputeId: Parameter
        + Member
        + AtLeast32Bit
        + Codec
        + Default
        + Copy
        + MaybeSerializeDeserialize
        + Debug
        + PartialOrd
        + PartialEq
        + Zero;

    /// The minimum amount for any dispute registered in this module
    type MinimumDisputeAmount: Get<BalanceOf<Self>>;
}

decl_event!(
    pub enum Event<T>
    where
        <T as System>::AccountId,
        <T as Org>::OrgId,
        <T as Vote>::VoteId,
        <T as Trait>::DisputeId,
        Balance = BalanceOf<T>,

    {
        RegisteredDisputeWithResolutionPath(DisputeId, AccountId, Balance, AccountId, OrgRep<OrgId>),
        DisputeRaisedAndVoteTriggered(DisputeId, AccountId, Balance, AccountId, OrgRep<OrgId>, VoteId),
        DisputeAcceptedAndLockedFundsTransferred(DisputeId, AccountId, Balance, AccountId, OrgId, VoteId),
        DisputeRejectedAndLockedFundsUnlocked(DisputeId, AccountId, Balance, AccountId, OrgId, VoteId),
    }
);

decl_error! {
    pub enum Error for Module<T: Trait> {
        /// Must register dispute with resolution path before raising one
        CannotRaiseDisputeIfDisputeStateDNE,
        DisputeMustExceedModuleMinimum,
        CannotPollDisputeIfDisputeStateDNE,
        SignerNotAuthorizedToRaiseThisDispute,
        ActiveDisputeCannotBeRaisedFromCurrentState,
        ActiveDisputeCannotBePolledFromCurrentState,
        VoteOutcomeInconclusiveSoPollCannotExecuteOutcome,
    }
}

decl_storage! {
    trait Store for Module<T: Trait> as Court {
        /// The nonce for unique dispute id generation
        DisputeIdCounter get(fn dispute_id_counter): T::DisputeId;

        /// The number of open disputes
        pub OpenDisputeCounter get(fn open_dispute_counter): u32;

        /// The state of disputes
        pub DisputeStates get(fn dispute_states): map
            hasher(blake2_128_concat) T::DisputeId => Option<DisputeOf<T>>;
    }
}

decl_module! {
    pub struct Module<T: Trait> for enum Call where origin: T::Origin {
        type Error = Error<T>;
        fn deposit_event() = default;

        #[weight = 0]
        fn register_dispute_type_with_resolution_path(
            origin,
            amount_to_lock: BalanceOf<T>,
            dispute_raiser: T::AccountId,
            resolution_metadata: GovernanceOf<T>,
            expiry: Option<T::BlockNumber>,
        ) -> DispatchResult {
            let locker = ensure_signed(origin)?;
            // get court org before new dispute state consumes resolution metadata
            let court_org = resolution_metadata.org();
            let new_dispute_id = Self::register_dispute_type(
                locker.clone(),
                amount_to_lock,
                dispute_raiser.clone(),
                resolution_metadata,
                expiry,
            )?;
            // emit the event with the DisputeId
            Self::deposit_event(RawEvent::RegisteredDisputeWithResolutionPath(new_dispute_id, locker, amount_to_lock, dispute_raiser, court_org));
            Ok(())
        }
        #[weight = 0]
        fn raise_dispute_to_trigger_vote(
            origin,
            dispute_id: T::DisputeId,
        ) -> DispatchResult {
            let trigger = ensure_signed(origin)?;
            let dispute = <DisputeStates<T>>::get(dispute_id).ok_or(Error::<T>::CannotRaiseDisputeIfDisputeStateDNE)?;
            // ensure that the signer can trigger this dispute
            ensure!(dispute.can_raise_dispute(&trigger), Error::<T>::SignerNotAuthorizedToRaiseThisDispute);
            // check that it is in a valid state to trigger the dispute
            let (new_dispute, dispatched_vote_id) = match dispute.state() {
                DisputeState::DisputeNotRaised => {
                    // use vote metadata to dispatch vote
                    let new_vote_id = match dispute.resolution_metadata() {
                        VoteMetadata::Signal(v) => <vote::Module<T>>::open_vote(None, v.org, v.threshold, v.duration)?,
                        VoteMetadata::Percentage(v) => <vote::Module<T>>::open_percent_vote(None, v.org, v.threshold, v.duration)?,
                    };
                    // update the state of the dispute with the new vote identifier
                    let updated_dispute = dispute.set_state(DisputeState::DisputeRaisedAndVoteDispatched(new_vote_id));
                    // return tuple
                    (updated_dispute, new_vote_id)
                },
                // throw error if not in a state to trigger vote
                _ => return Err(Error::<T>::ActiveDisputeCannotBeRaisedFromCurrentState.into()),
            };
            let (locker, amt_locked, court_org) = (
                new_dispute.locker(),
                new_dispute.locked_funds(),
                new_dispute.resolution_metadata().org(),
            );
            // insert new dispute state
            <DisputeStates<T>>::insert(dispute_id, new_dispute);
            // emit the event with the VoteId
            Self::deposit_event(RawEvent::DisputeRaisedAndVoteTriggered(dispute_id, locker, amt_locked, trigger, court_org, dispatched_vote_id));
            Ok(())
        }
        #[weight = 0]
        fn poll_dispute_to_execute_outcome(
            origin,
            dispute_id: T::DisputeId,
        ) -> DispatchResult {
            let _ = ensure_signed(origin)?;
            let dispute = <DisputeStates<T>>::get(dispute_id).ok_or(Error::<T>::CannotPollDisputeIfDisputeStateDNE)?;
            // _could_ verify poller in context of dispute here

            // match on the dispute's current state
            let new_dispute_state = match dispute.state() {
                DisputeState::DisputeRaisedAndVoteDispatched(live_vote_id) => {
                    // check the vote outcome
                    let outcome = <vote::Module<T>>::get_vote_outcome(live_vote_id)?;
                    match outcome {
                        VoteOutcome::Approved => {
                            // unreserve capital from locker
                            let _ = T::Currency::unreserve(&dispute.locker(), dispute.locked_funds());
                            // transfer from locker to dispute_raiser
                            T::Currency::transfer(&dispute.locker(), &dispute.dispute_raiser(), dispute.locked_funds(), ExistenceRequirement::KeepAlive)?;
                            // update dispute state
                            dispute.set_state(DisputeState::DisputeRaisedAndAccepted(live_vote_id))
                        }
                        VoteOutcome::Rejected => {
                            // unreserve capital from locker
                            let _ = T::Currency::unreserve(&dispute.locker(), dispute.locked_funds());
                            // update dispute state
                            dispute.set_state(DisputeState::DisputeRaisedAndRejected(live_vote_id))
                        }
                        _ => return Err(Error::<T>::VoteOutcomeInconclusiveSoPollCannotExecuteOutcome.into()),
                    }
                }
                _ => return Err(Error::<T>::ActiveDisputeCannotBePolledFromCurrentState.into()),
            };
            // insert new dispute state
            <DisputeStates<T>>::insert(dispute_id, new_dispute_state);
            // emit the event with the outcome
            Ok(())
        }
    }
}

impl<T: Trait> IDIsAvailable<T::DisputeId> for Module<T> {
    fn id_is_available(id: T::DisputeId) -> bool {
        <DisputeStates<T>>::get(id).is_none()
    }
}

impl<T: Trait> GenerateUniqueID<T::DisputeId> for Module<T> {
    fn generate_unique_id() -> T::DisputeId {
        let mut id_counter = <DisputeIdCounter<T>>::get() + 1u32.into();
        while <DisputeStates<T>>::get(id_counter).is_some() {
            id_counter += 1u32.into();
        }
        <DisputeIdCounter<T>>::put(id_counter);
        id_counter
    }
}

impl<T: Trait>
    RegisterDisputeType<
        T::AccountId,
        BalanceOf<T>,
        GovernanceOf<T>,
        T::BlockNumber,
    > for Module<T>
{
    type DisputeIdentifier = T::DisputeId;
    fn register_dispute_type(
        locker: T::AccountId,
        amount_to_lock: BalanceOf<T>,
        dispute_raiser: T::AccountId,
        resolution_path: GovernanceOf<T>,
        expiry: Option<T::BlockNumber>,
    ) -> Result<Self::DisputeIdentifier, DispatchError> {
        ensure!(
            amount_to_lock >= T::MinimumDisputeAmount::get(),
            Error::<T>::DisputeMustExceedModuleMinimum
        );
        // lock the amount in question
        T::Currency::reserve(&locker, amount_to_lock)?;
        // form the dispute state
        let new_dispute_state = Dispute::new(
            locker,
            amount_to_lock,
            dispute_raiser,
            resolution_path,
            DisputeState::DisputeNotRaised,
            expiry,
        );
        // generate unique dispute identifier
        let new_dispute_id = Self::generate_unique_id();
        // insert the dispute state
        <DisputeStates<T>>::insert(new_dispute_id, new_dispute_state);
        Ok(new_dispute_id)
    }
}
