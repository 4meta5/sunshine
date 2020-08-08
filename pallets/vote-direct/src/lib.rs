#![recursion_limit = "256"]
#![allow(clippy::string_lit_as_bytes)]
#![allow(clippy::redundant_closure_call)]
#![allow(clippy::type_complexity)]
#![allow(clippy::too_many_arguments)]
#![cfg_attr(not(feature = "std"), no_std)]
//! Voting from sets of weighted accounts

#[cfg(test)]
mod tests;

use codec::Codec;
use frame_support::{
    decl_error,
    decl_event,
    decl_module,
    decl_storage,
    ensure,
    Parameter,
};
use frame_system::{
    self as system,
    ensure_signed,
};
use sp_runtime::{
    traits::{
        AtLeast32BitUnsigned,
        CheckedSub,
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
    share::SimpleShareGenesis,
    traits::{
        AccessGenesis,
        Apply,
        ApplyVote,
        CheckVoteStatus,
        GenerateUniqueID,
        GetVoteOutcome,
        IDIsAvailable,
        OpenVote,
        UpdateVoteTopic,
        VoteOnProposal,
        VoteVector,
    },
    vote::{
        Threshold,
        Vote,
        VoteOutcome,
        VoteState,
        VoterView,
    },
};

// type aliases
type VoteSt<T> = VoteState<
    <T as Trait>::Signal,
    <T as frame_system::Trait>::BlockNumber,
    <T as Trait>::IpfsReference,
>;
type VoteVec<T> = Vote<<T as Trait>::Signal, <T as Trait>::IpfsReference>;

pub trait Trait: frame_system::Trait {
    /// The overarching event type
    type Event: From<Event<Self>> + Into<<Self as frame_system::Trait>::Event>;

    /// Cid type
    type IpfsReference: Parameter + Copy;

    /// The vote identifier
    type VoteId: Parameter
        + Member
        + AtLeast32BitUnsigned
        + Codec
        + Default
        + Copy
        + MaybeSerializeDeserialize
        + Debug
        + PartialOrd
        + PartialEq
        + Zero;

    /// The metric for voting power
    type Signal: Parameter
        + Member
        + AtLeast32BitUnsigned
        + Codec
        + Default
        + Copy
        + MaybeSerializeDeserialize
        + Debug
        + PartialOrd
        + CheckedSub
        + Zero;
}

decl_event!(
    pub enum Event<T>
    where
        <T as frame_system::Trait>::AccountId,
        <T as Trait>::VoteId,
    {
        NewVoteStarted(AccountId, VoteId),
        Voted(VoteId, AccountId, VoterView),
    }
);

decl_error! {
    pub enum Error for Module<T: Trait> {
        VotePastExpirationTimeSoVotesNotAccepted,
        SignalNotMintedForVoter,
        NoVoteStateForOutcomeQuery,
        NoVoteStateForVoteRequest,
        OldVoteDirectionEqualsNewVoteDirectionSoNoChange,
        CannotUpdateVoteTopicIfVoteStateDNE,
        // i.e. changing from any non-NoVote view to NoVote (some vote changes aren't allowed to simplify assumptions)
        VoteChangeNotSupported,
        InvalidVoteGenesisInput,
        InputThresholdExceedsBounds,
    }
}

decl_storage! {
    trait Store for Module<T: Trait> as Vote {
        /// The nonce for unique vote id generation
        VoteIdCounter get(fn vote_id_counter): T::VoteId;

        /// The number of open votes
        pub OpenVoteCounter get(fn open_vote_counter): u32;

        /// The state of a vote
        pub VoteStates get(fn vote_states): map
            hasher(opaque_blake2_256) T::VoteId => Option<VoteSt<T>>;

        /// Tracks all votes and signal for each participating account
        pub VoteLogger get(fn vote_logger): double_map
            hasher(opaque_blake2_256) T::VoteId,
            hasher(opaque_blake2_256) T::AccountId  => Option<VoteVec<T>>;
    }
}

decl_module! {
    pub struct Module<T: Trait> for enum Call where origin: T::Origin {
        type Error = Error<T>;
        fn deposit_event() = default;

        #[weight = 0]
        pub fn create_signal_vote(
            origin,
            topic: Option<T::IpfsReference>,
            src: SimpleShareGenesis<T::AccountId, T::Signal>,
            threshold: Threshold<T::Signal>,
            duration: Option<T::BlockNumber>,
        ) -> DispatchResult {
            let vote_creator = ensure_signed(origin)?;
            // call helper method
            let vote_id = Self::open_vote(
                topic,
                src,
                threshold,
                duration,
            )?;
            // emit event
            Self::deposit_event(RawEvent::NewVoteStarted(vote_creator, vote_id));
            Ok(())
        }
        #[weight = 0]
        pub fn create_percent_vote(
            origin,
            topic: Option<T::IpfsReference>,
            src: SimpleShareGenesis<T::AccountId, T::Signal>,
            threshold: Threshold<Permill>,
            duration: Option<T::BlockNumber>,
        ) -> DispatchResult {
            let vote_creator = ensure_signed(origin)?;
            // call helper method
            let vote_id = Self::open_percent_vote(
                topic,
                src,
                threshold,
                duration,
            )?;
            // emit event
            Self::deposit_event(RawEvent::NewVoteStarted(vote_creator, vote_id));
            Ok(())
        }
        #[weight = 0]
        pub fn submit_vote(
            origin,
            vote_id: T::VoteId,
            direction: VoterView,
            justification: Option<T::IpfsReference>,
        ) -> DispatchResult {
            let voter = ensure_signed(origin)?;
            Self::vote_on_proposal(vote_id, voter.clone(), direction, justification)?;
            Self::deposit_event(RawEvent::Voted(vote_id, voter, direction));
            Ok(())
        }
    }
}

impl<T: Trait> Module<T> {
    fn valid_signal_threshold(
        threshold: &Threshold<T::Signal>,
        all_possible_turnout: T::Signal,
    ) -> bool {
        threshold.in_favor() <= all_possible_turnout
            && (if let Some(t) = threshold.against() {
                t <= all_possible_turnout
            } else {
                true
            })
    }
    fn from_permill_to_signal(
        threshold: &Threshold<Permill>,
        all_possible_turnout: T::Signal,
    ) -> Threshold<T::Signal> {
        let in_favor_t: T::Signal = threshold.in_favor() * all_possible_turnout;
        let against_t: Option<T::Signal> = if let Some(t) = threshold.against()
        {
            Some(t * all_possible_turnout)
        } else {
            None
        };
        Threshold::new(in_favor_t, against_t)
    }
}

impl<T: Trait> IDIsAvailable<T::VoteId> for Module<T> {
    fn id_is_available(id: T::VoteId) -> bool {
        <VoteStates<T>>::get(id).is_none()
    }
}

impl<T: Trait> GenerateUniqueID<T::VoteId> for Module<T> {
    fn generate_unique_id() -> T::VoteId {
        let mut id_counter = <VoteIdCounter<T>>::get() + 1u32.into();
        while <VoteStates<T>>::get(id_counter).is_some() {
            id_counter += 1u32.into();
        }
        <VoteIdCounter<T>>::put(id_counter);
        id_counter
    }
}

impl<T: Trait> GetVoteOutcome<T::VoteId> for Module<T> {
    type Outcome = VoteOutcome;
    fn get_vote_outcome(
        vote_id: T::VoteId,
    ) -> Result<Self::Outcome, DispatchError> {
        let vote_state = <VoteStates<T>>::get(vote_id)
            .ok_or(Error::<T>::NoVoteStateForOutcomeQuery)?;
        Ok(vote_state.outcome())
    }
}

impl<T: Trait>
    OpenVote<
        SimpleShareGenesis<T::AccountId, T::Signal>,
        Threshold<T::Signal>,
        Threshold<Permill>,
        T::BlockNumber,
        T::IpfsReference,
    > for Module<T>
{
    type VoteIdentifier = T::VoteId;
    fn open_vote(
        topic: Option<T::IpfsReference>,
        src: SimpleShareGenesis<T::AccountId, T::Signal>,
        threshold: Threshold<T::Signal>,
        duration: Option<T::BlockNumber>,
    ) -> Result<Self::VoteIdentifier, DispatchError> {
        let vote_id = Self::generate_unique_id();
        // iterate through src and mint the signal
        let mut expected_total: T::Signal = 0u32.into();
        src.account_ownership()
            .iter()
            .for_each(|(who, vote_power)| {
                let new_vote =
                    Vote::new(vote_power.clone(), VoterView::NoVote, None);
                <VoteLogger<T>>::insert(vote_id, who, new_vote);
                expected_total += *vote_power
            });
        // validate that the total is actually the total and throw error if not
        ensure!(
            expected_total == src.total(),
            Error::<T>::InvalidVoteGenesisInput
        );
        ensure!(
            Self::valid_signal_threshold(&threshold, expected_total),
            Error::<T>::InputThresholdExceedsBounds
        );
        let now = system::Module::<T>::block_number();
        let ends: Option<T::BlockNumber> = if let Some(time_to_add) = duration {
            Some(now + time_to_add)
        } else {
            None
        };
        let new_vote_state =
            VoteState::new(topic, src.total(), threshold, now, ends);
        // insert the VoteState
        <VoteStates<T>>::insert(vote_id, new_vote_state);
        // increment open vote count
        let new_vote_count = <OpenVoteCounter>::get() + 1u32;
        <OpenVoteCounter>::put(new_vote_count);
        Ok(vote_id)
    }
    fn open_percent_vote(
        topic: Option<T::IpfsReference>,
        src: SimpleShareGenesis<T::AccountId, T::Signal>,
        threshold: Threshold<Permill>,
        duration: Option<T::BlockNumber>,
    ) -> Result<Self::VoteIdentifier, DispatchError> {
        let signal_threshold =
            Self::from_permill_to_signal(&threshold, src.total());
        let vote_id = Self::generate_unique_id();
        // iterate through src and mint the signal
        let mut expected_total: T::Signal = 0u32.into();
        src.account_ownership()
            .iter()
            .for_each(|(who, vote_power)| {
                let new_vote =
                    Vote::new(vote_power.clone(), VoterView::NoVote, None);
                <VoteLogger<T>>::insert(vote_id, who, new_vote);
                expected_total += *vote_power
            });
        // validate that the total is actually the total and throw error if not
        ensure!(
            expected_total == src.total(),
            Error::<T>::InvalidVoteGenesisInput
        );
        ensure!(
            Self::valid_signal_threshold(&signal_threshold, expected_total),
            Error::<T>::InputThresholdExceedsBounds
        );
        let now = system::Module::<T>::block_number();
        let ends: Option<T::BlockNumber> = if let Some(time_to_add) = duration {
            Some(now + time_to_add)
        } else {
            None
        };
        let new_vote_state =
            VoteState::new(topic, src.total(), signal_threshold, now, ends);
        // insert the VoteState
        <VoteStates<T>>::insert(vote_id, new_vote_state);
        // increment open vote count
        let new_vote_count = <OpenVoteCounter>::get() + 1u32;
        <OpenVoteCounter>::put(new_vote_count);
        Ok(vote_id)
    }
}

impl<T: Trait> UpdateVoteTopic<T::VoteId, T::IpfsReference> for Module<T> {
    fn update_vote_topic(
        vote_id: T::VoteId,
        new_topic: T::IpfsReference,
        clear_previous_vote_state: bool,
    ) -> DispatchResult {
        let old_vote_state = <VoteStates<T>>::get(vote_id)
            .ok_or(Error::<T>::CannotUpdateVoteTopicIfVoteStateDNE)?;
        let new_vote_state = if clear_previous_vote_state {
            old_vote_state.update_topic_and_clear_state(new_topic)
        } else {
            old_vote_state.update_topic_without_clearing_state(new_topic)
        };
        <VoteStates<T>>::insert(vote_id, new_vote_state);
        Ok(())
    }
}

impl<T: Trait> ApplyVote<T::IpfsReference> for Module<T> {
    type Signal = T::Signal;
    type Direction = VoterView;
    type Vote = Vote<T::Signal, T::IpfsReference>;
    type State = VoteState<T::Signal, T::BlockNumber, T::IpfsReference>;

    fn apply_vote(
        state: Self::State,
        vote_magnitude: T::Signal,
        old_vote_view: Self::Direction,
        new_vote_view: Self::Direction,
    ) -> Option<Self::State> {
        state.apply(vote_magnitude, old_vote_view, new_vote_view)
    }
}

impl<T: Trait> CheckVoteStatus<T::IpfsReference, T::VoteId> for Module<T> {
    fn check_vote_expired(state: &Self::State) -> bool {
        let now = system::Module::<T>::block_number();
        if let Some(n) = state.expires() {
            return n < now
        }
        false
    }
}

impl<T: Trait> VoteOnProposal<T::AccountId, T::VoteId, T::IpfsReference>
    for Module<T>
{
    fn vote_on_proposal(
        vote_id: T::VoteId,
        voter: T::AccountId,
        direction: Self::Direction,
        justification: Option<T::IpfsReference>,
    ) -> DispatchResult {
        // get the vote state
        let vote_state = <VoteStates<T>>::get(vote_id)
            .ok_or(Error::<T>::NoVoteStateForVoteRequest)?;
        // TODO: add permissioned method for adding time to the vote state because of this restriction but this is a legitimate restriction
        // -> every standard vote has a recognized end to establish when the decision must be made based on collected input
        ensure!(
            !Self::check_vote_expired(&vote_state),
            Error::<T>::VotePastExpirationTimeSoVotesNotAccepted
        );
        // get the organization associated with this vote_state
        let old_vote = <VoteLogger<T>>::get(vote_id, voter.clone())
            .ok_or(Error::<T>::SignalNotMintedForVoter)?;
        let new_vote = old_vote.set_new_view(direction, justification).ok_or(
            Error::<T>::OldVoteDirectionEqualsNewVoteDirectionSoNoChange,
        )?;
        let new_state = Self::apply_vote(
            vote_state,
            old_vote.magnitude(),
            old_vote.direction(),
            direction,
        )
        .ok_or(Error::<T>::VoteChangeNotSupported)?;
        // set the new vote for the voter's profile
        <VoteLogger<T>>::insert(vote_id, voter, new_vote);
        // commit new vote state to storage
        <VoteStates<T>>::insert(vote_id, new_state);
        Ok(())
    }
}
