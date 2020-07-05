use crate::srml::org::{
    Org,
    OrgEventsDecoder,
};
use codec::{
    Codec,
    Decode,
    Encode,
};
use frame_support::Parameter;
use sp_runtime::traits::{
    AtLeast32Bit,
    MaybeSerializeDeserialize,
    Member,
    Zero,
};
use std::fmt::Debug;
use substrate_subxt::system::{
    System,
    SystemEventsDecoder,
};
use util::bank::{
    BankOrAccount,
    BankState,
    OnChainTreasuryID,
};

pub type BalanceOf<T> = <T as Bank>::Currency; // as Currency<<T as System>::AccountId>>::Balance;

/// The subset of the bank trait and its inherited traits that the client must inherit
#[module]
pub trait Bank: System + Org {
    /// The currency type for on-chain transactions
    type Currency: Parameter
        + Member
        + AtLeast32Bit
        + Codec
        + Default
        + Copy
        + MaybeSerializeDeserialize
        + Debug
        + PartialOrd
        + PartialEq
        + Zero; // + Currency<<Self as System>::AccountId> // commented out until #93 is resolved
}

// ~~ Values (Constants) ~~

#[derive(Clone, Debug, Eq, PartialEq, Encode)]
pub struct MinimumInitialDepositStore<T: Bank> {
    pub amount: BalanceOf<T>,
}

#[derive(Clone, Debug, Eq, PartialEq, Encode)]
pub struct MinimumTransferStore<T: Bank> {
    pub amount: BalanceOf<T>,
}

// ~~ Maps ~~

#[derive(Clone, Debug, Eq, PartialEq, Store, Encode)]
pub struct BankStoresStore<T: Bank> {
    #[store(returns = BankState<<T as System>::AccountId, <T as Org>::OrgId, BalanceOf<T>>)]
    pub id: OnChainTreasuryID,
    phantom: std::marker::PhantomData<T>,
}

#[derive(Clone, Debug, Eq, PartialEq, Store, Encode)]
pub struct SpendReservationsStore<T: Bank> {
    #[store(returns = SpendReservation<BankOrAccount<OnChainTreasuryID, <T as System>::AccountId>, BalanceOf<T>>)]
    pub bank_id: OnChainTreasuryID,
    pub reservation_id: T::BankId,
}

#[derive(Clone, Debug, Eq, PartialEq, Store, Encode)]
pub struct InternalTransfersStore<T: Bank> {
    #[store(returns = TransferInformation<OrgOrAccount<<T as Org>::OrgId, <T as System>::AccountId>, BalanceOf<T>, TransferState>)]
    pub bank_id: OnChainTreasuryID,
    pub transfer_id: T::BankId,
}

// ~~ (Calls, Events) ~~

#[derive(Clone, Debug, Eq, PartialEq, Call, Encode)]
pub struct DepositFromSignerForBankAccountCall<T: Bank> {
    pub bank_id: OnChainTreasuryID,
    pub amount: BalanceOf<T>,
    pub reason: <T as Org>::IpfsReference,
}

#[derive(Clone, Debug, Eq, PartialEq, Event, Decode)]
pub struct CapitalDepositedIntoOnChainBankAccountEvent<T: Bank> {
    pub depositer: <T as System>::AccountId,
    pub bank_id: OnChainTreasuryID,
    pub amount: BalanceOf<T>,
    pub reason: <T as Org>::IpfsReference,
}

#[derive(Clone, Debug, Eq, PartialEq, Call, Encode)]
pub struct RegisterAndSeedForBankAccountCall<T: Bank> {
    pub seed: BalanceOf<T>,
    pub hosting_org: <T as Org>::OrgId,
    pub bank_operator: Option<<T as Org>::OrgId>,
}

#[derive(Clone, Debug, Eq, PartialEq, Event, Decode)]
pub struct RegisteredNewOnChainBankEvent<T: Bank> {
    pub seeder: <T as System>::AccountId,
    pub new_bank_id: OnChainTreasuryID,
    pub seed: BalanceOf<T>,
    pub hosting_org: <T as Org>::OrgId,
    pub bank_operator: Option<<T as Org>::OrgId>,
}

#[derive(Clone, Debug, Eq, PartialEq, Call, Encode)]
pub struct ReserveSpendForBankAccountCall<T: Bank> {
    pub bank_id: OnChainTreasuryID,
    pub reason: <T as Org>::IpfsReference,
    pub amount: BalanceOf<T>,
    pub controller: <T as Org>::OrgId,
}

#[derive(Clone, Debug, Eq, PartialEq, Event, Decode)]
pub struct SpendReservedForBankAccountEvent<T: Bank> {
    pub bank_id: OnChainTreasuryID,
    pub new_reservation_id: T::BankId,
    pub reason: <T as Org>::IpfsReference,
    pub amount: BalanceOf<T>,
    pub controller: <T as Org>::OrgId,
}

#[derive(Clone, Debug, Eq, PartialEq, Call, Encode)]
pub struct CommitReserveSpendForTransferInsideBankAccountCall<T: Bank> {
    pub bank_id: OnChainTreasuryID,
    pub reservation_id: T::BankId,
    pub reason: <T as Org>::IpfsReference,
    pub amount: BalanceOf<T>,
}

#[derive(Clone, Debug, Eq, PartialEq, Event, Decode)]
pub struct CommitSpendBeforeInternalTransferEvent<T: Bank> {
    pub committer: <T as System>::AccountId,
    pub bank_id: OnChainTreasuryID,
    pub reservation_id: T::BankId,
    pub amount: BalanceOf<T>,
}

#[derive(Clone, Debug, Eq, PartialEq, Call, Encode)]
pub struct UnreserveUncommittedReservationToMakeFreeCall<T: Bank> {
    pub bank_id: OnChainTreasuryID,
    pub reservation_id: T::BankId,
    pub amount: BalanceOf<T>,
}

#[derive(Clone, Debug, Eq, PartialEq, Event, Decode)]
pub struct UnreserveUncommittedReservationToMakeFreeEvent<T: Bank> {
    pub qualified_bank_controller: <T as System>::AccountId,
    pub bank_id: OnChainTreasuryID,
    pub reservation_id: T::BankId,
    pub amount: BalanceOf<T>,
}

#[derive(Clone, Debug, Eq, PartialEq, Call, Encode)]
pub struct UnreserveCommittedReservationToMakeFreeCall<T: Bank> {
    pub bank_id: OnChainTreasuryID,
    pub reservation_id: T::BankId,
    pub amount: BalanceOf<T>,
}

#[derive(Clone, Debug, Eq, PartialEq, Event, Decode)]
pub struct UnreserveCommittedReservationToMakeFreeEvent<T: Bank> {
    pub qualified_spend_reservation_controller: <T as System>::AccountId,
    pub bank_id: OnChainTreasuryID,
    pub reservation_id: T::BankId,
    pub amount: BalanceOf<T>,
}

#[derive(Clone, Debug, Eq, PartialEq, Call, Encode)]
pub struct TransferSpendingPowerForSpendCommitmentCall<T: Bank> {
    pub bank_id: OnChainTreasuryID,
    pub reason: <T as Org>::IpfsReference,
    pub reservation_id: T::BankId,
    pub amount: BalanceOf<T>,
    pub committed_controller: <T as Org>::OrgId,
}

#[derive(Clone, Debug, Eq, PartialEq, Event, Decode)]
pub struct InternalTransferExecutedAndSpendingPowerDoledOutToControllerEvent<
    T: Bank,
> {
    pub qualified_spend_reservation_controller: <T as System>::AccountId,
    pub bank_id: OnChainTreasuryID,
    pub reason: <T as Org>::IpfsReference,
    pub reservation_id: T::BankId,
    pub amount: BalanceOf<T>,
    pub committed_controller: <T as Org>::OrgId,
}

#[derive(Clone, Debug, Eq, PartialEq, Call, Encode)]
pub struct WithdrawByReferencingInternalTransferCall<T: Bank> {
    pub bank_id: OnChainTreasuryID,
    pub transfer_id: T::BankId,
    pub amount: BalanceOf<T>,
}

#[derive(Clone, Debug, Eq, PartialEq, Event, Decode)]
pub struct SpendRequestForInternalTransferApprovedAndExecutedEvent<T: Bank> {
    pub bank_id: OnChainTreasuryID,
    pub requester: <T as System>::AccountId,
    pub amount: BalanceOf<T>,
    pub transfer_id: T::BankId,
}

#[derive(Clone, Debug, Eq, PartialEq, Call, Encode)]
pub struct BurnAllSharesToLeaveWeightedMembershipBankAndWithdrawRelatedFreeCapitalCall<
    T: Bank,
> {
    pub bank_id: OnChainTreasuryID,
    phantom: std::marker::PhantomData<T>,
}

#[derive(Clone, Debug, Eq, PartialEq, Event, Decode)]
pub struct AccountLeftMembershipAndWithdrewProportionOfFreeCapitalInBankEvent<
    T: Bank,
> {
    pub bank_id: OnChainTreasuryID,
    pub leaving_member: <T as System>::AccountId,
    pub amount_withdrawn_by_burning_shares: BalanceOf<T>,
}
