#![cfg_attr(not(feature = "std"), no_std)]

pub use pallet::*;

#[rustfmt::skip]
pub mod weights;

#[cfg(test)]
mod mock;

#[cfg(test)]
mod tests;

#[cfg(feature = "runtime-benchmarks")]
mod benchmarking;

use frame_support::{
    dispatch::DispatchResult,
    ensure,
    traits::{
        tokens::{
            fungibles::{
                metadata::Mutate as FungMetaMutate, Create as FungCreate, Mutate as FungMutate,
                Transfer as FungTransfer,
            },
            nonfungibles::{Create as NftCreate, Mutate as NftMutate},
        },
        Currency, EnsureOrigin,
        ExistenceRequirement::KeepAlive,
        Get,
    },
};
use parami_did::{EnsureDid, Pallet as Did};
use parami_traits::Swaps;
use sp_core::U512;
use sp_runtime::{
    traits::{Bounded, CheckedAdd, One, Saturating},
    DispatchError,
};
use sp_std::{
    convert::{TryFrom, TryInto},
    prelude::*,
};

use weights::WeightInfo;

type AccountOf<T> = <T as frame_system::Config>::AccountId;
type BalanceOf<T> = <<T as parami_did::Config>::Currency as Currency<AccountOf<T>>>::Balance;
type HeightOf<T> = <T as frame_system::Config>::BlockNumber;

#[frame_support::pallet]
pub mod pallet {
    use super::*;
    use frame_support::pallet_prelude::*;
    use frame_system::pallet_prelude::*;

    #[pallet::config]
    pub trait Config: frame_system::Config + parami_did::Config {
        /// The overarching event type
        type Event: From<Event<Self>> + IsType<<Self as frame_system::Config>::Event>;

        /// The assets trait to create, mint, and transfer fragments (fungible token)
        /// it uses parami_did::Config::AssetId as AssetId
        type Assets: FungCreate<AccountOf<Self>, AssetId = Self::AssetId>
            + FungMetaMutate<AccountOf<Self>, AssetId = Self::AssetId>
            + FungMutate<AccountOf<Self>, AssetId = Self::AssetId, Balance = BalanceOf<Self>>
            + FungTransfer<AccountOf<Self>, AssetId = Self::AssetId, Balance = BalanceOf<Self>>;

        /// The ICO baseline of donation for currency
        #[pallet::constant]
        type InitialMintingDeposit: Get<BalanceOf<Self>>;

        /// The ICO lockup period for fragments, KOL will not be able to claim before this period
        #[pallet::constant]
        type InitialMintingLockupPeriod: Get<HeightOf<Self>>;

        /// The ICO value base of fragments, system will mint triple of the value
        /// once for KOL, once to swaps, once to supporters
        /// The maximum value of fragments is decuple of this value
        #[pallet::constant]
        type InitialMintingValueBase: Get<BalanceOf<Self>>;

        /// The NFT trait to create, mint non-fungible token
        /// it uses parami_did::Config::AssetId as InstanceId and ClassId
        type Nft: NftCreate<AccountOf<Self>, InstanceId = Self::AssetId, ClassId = Self::AssetId>
            + NftMutate<AccountOf<Self>, InstanceId = Self::AssetId, ClassId = Self::AssetId>;

        /// The maximum length of a name or symbol stored on-chain.
        #[pallet::constant]
        type StringLimit: Get<u32>;

        /// The swaps trait
        type Swaps: Swaps<
            AccountId = AccountOf<Self>,
            AssetId = Self::AssetId,
            QuoteBalance = BalanceOf<Self>,
            TokenBalance = BalanceOf<Self>,
        >;

        /// Weight information for extrinsics in this pallet.
        type WeightInfo: WeightInfo;
    }

    #[pallet::pallet]
    #[pallet::generate_store(pub(super) trait Store)]
    pub struct Pallet<T>(_);

    /// Total deposit in pot
    #[pallet::storage]
    #[pallet::getter(fn deposit)]
    pub(super) type Deposit<T: Config> = StorageMap<
        _,
        Identity,
        T::DecentralizedId, // KOL
        BalanceOf<T>,
    >;

    /// Deposits by supporter in pot
    #[pallet::storage]
    #[pallet::getter(fn deposits)]
    pub(super) type Deposits<T: Config> = StorageDoubleMap<
        _,
        Identity,
        T::DecentralizedId, // KOL
        Identity,
        T::DecentralizedId, // Supporter
        BalanceOf<T>,
    >;

    /// Initial Minting date
    #[pallet::storage]
    #[pallet::getter(fn date)]
    pub(super) type Date<T: Config> = StorageMap<_, Twox64Concat, T::AssetId, HeightOf<T>>;

    /// Next available class ID
    #[pallet::storage]
    #[pallet::getter(fn next_cid)]
    pub(super) type NextClassId<T: Config> = StorageValue<_, T::AssetId, ValueQuery>;

    #[pallet::event]
    #[pallet::generate_deposit(pub(super) fn deposit_event)]
    pub enum Event<T: Config> {
        /// NFT fragments Minted \[did, kol, value\]
        Backed(T::DecentralizedId, T::DecentralizedId, BalanceOf<T>),
        /// NFT fragments Claimed \[did, kol, value\]
        Claimed(T::DecentralizedId, T::DecentralizedId, BalanceOf<T>),
        /// NFT fragments Minted \[kol, class, instance, name, symbol, tokens\]
        Minted(
            T::DecentralizedId,
            T::AssetId,
            T::AssetId,
            Vec<u8>,
            Vec<u8>,
            BalanceOf<T>,
        ),
    }

    #[pallet::hooks]
    impl<T: Config> Hooks<HeightOf<T>> for Pallet<T> {}

    #[pallet::error]
    pub enum Error<T> {
        BadMetadata,
        InsufficientBalance,
        Minted,
        Overflow,
        NotExists,
        NoToken,
        YourSelf,
    }

    #[pallet::call]
    impl<T: Config> Pallet<T> {
        /// Back (support) the KOL.
        #[pallet::weight(<T as Config>::WeightInfo::back())]
        pub fn back(
            origin: OriginFor<T>,
            kol: T::DecentralizedId,
            #[pallet::compact] value: BalanceOf<T>,
        ) -> DispatchResult {
            let (did, who) = EnsureDid::<T>::ensure_origin(origin)?;

            ensure!(kol != did, Error::<T>::YourSelf);

            let meta = Did::<T>::meta(&kol).ok_or(Error::<T>::NotExists)?;

            ensure!(meta.nft.is_none(), Error::<T>::Minted);

            <T as parami_did::Config>::Currency::transfer(&who, &meta.pot, value, KeepAlive)?;

            <Deposit<T>>::mutate(&kol, |maybe| {
                if let Some(deposit) = maybe {
                    deposit.saturating_accrue(value);
                } else {
                    *maybe = Some(value);
                }
            });

            <Deposits<T>>::mutate(&kol, &did, |maybe| {
                if let Some(deposit) = maybe {
                    deposit.saturating_accrue(value);
                } else {
                    *maybe = Some(value);
                }
            });

            Self::deposit_event(Event::Backed(did, kol, value));

            Ok(())
        }

        /// Fragment the NFT and mint token.
        #[pallet::weight(<T as Config>::WeightInfo::mint(
            name.len() as u32,
            symbol.len() as u32
        ))]
        pub fn mint(origin: OriginFor<T>, name: Vec<u8>, symbol: Vec<u8>) -> DispatchResult {
            let limit = T::StringLimit::get() as usize - 4;

            ensure!(
                0 < name.len() && name.len() <= limit,
                Error::<T>::BadMetadata
            );
            ensure!(
                0 < name.len() && symbol.len() <= limit,
                Error::<T>::BadMetadata
            );

            let is_valid_char = |c: &u8| c.is_ascii_whitespace() || c.is_ascii_alphanumeric();

            ensure!(
                name[0].is_ascii_alphabetic() && name.iter().all(is_valid_char),
                Error::<T>::BadMetadata
            );
            ensure!(
                symbol[0].is_ascii_alphabetic() && symbol.iter().all(is_valid_char),
                Error::<T>::BadMetadata
            );

            let minted = <frame_system::Pallet<T>>::block_number();

            let (did, _) = EnsureDid::<T>::ensure_origin(origin)?;

            // 1. ensure funded

            let mut meta = Did::<T>::meta(&did).ok_or(Error::<T>::NotExists)?;

            ensure!(meta.nft.is_none(), Error::<T>::Minted);

            let deposit = <T as parami_did::Config>::Currency::free_balance(&meta.pot);

            ensure!(
                deposit >= T::InitialMintingDeposit::get(),
                Error::<T>::InsufficientBalance
            );

            // 2. create NFT token

            let cid = NextClassId::<T>::try_mutate(|id| -> Result<T::AssetId, DispatchError> {
                let current_id = *id;
                *id = id.checked_add(&One::one()).ok_or(Error::<T>::Overflow)?;
                Ok(current_id)
            })?;

            let tid = T::AssetId::min_value();

            T::Nft::create_class(&cid, &meta.pot, &meta.pot)?;
            T::Nft::mint_into(&cid, &tid, &meta.pot)?;

            // 3. initial minting

            let initial = T::InitialMintingValueBase::get();

            T::Assets::create(cid, meta.pot.clone(), true, One::one())?;
            T::Assets::set(cid, &meta.pot, name.clone(), symbol.clone(), 18)?;
            T::Assets::mint_into(cid, &meta.pot, initial.saturating_mul(3u32.into()))?;

            // 4. transfer third of initial minting to swap

            T::Swaps::new(cid)?;
            T::Swaps::mint(meta.pot.clone(), cid, deposit, deposit, initial, false)?;

            meta.nft = Some(cid);

            Did::<T>::set_meta(&did, meta);

            <Date<T>>::insert(cid, minted);

            <Deposits<T>>::mutate(&did, &did, |maybe| {
                *maybe = Some(deposit);
            });

            Self::deposit_event(Event::Minted(did, cid, tid, name, symbol, initial));

            Ok(())
        }

        /// Claim the fragments.
        #[pallet::weight(<T as Config>::WeightInfo::claim())]
        pub fn claim(origin: OriginFor<T>, kol: T::DecentralizedId) -> DispatchResult {
            let (did, who) = EnsureDid::<T>::ensure_origin(origin)?;

            let height = <frame_system::Pallet<T>>::block_number();

            let meta = Did::<T>::meta(&kol).ok_or(Error::<T>::NotExists)?;

            let cid = meta.nft.ok_or(Error::<T>::NotExists)?;

            if kol == did {
                let minted_block_number = <Date<T>>::get(cid).ok_or(Error::<T>::NotExists)?;
                ensure!(
                    height - minted_block_number >= T::InitialMintingLockupPeriod::get(),
                    Error::<T>::NoToken
                );
            }

            let total = <Deposit<T>>::get(&kol).ok_or(Error::<T>::NotExists)?;
            let deposit = <Deposits<T>>::get(&kol, &did).ok_or(Error::<T>::NoToken)?;
            let initial = T::InitialMintingValueBase::get();

            let total: U512 = Self::try_into(total)?;
            let deposit: U512 = Self::try_into(deposit)?;
            let initial: U512 = Self::try_into(initial)?;

            let tokens = initial * deposit / total;

            let tokens = Self::try_into(tokens)?;

            T::Assets::transfer(cid, &meta.pot, &who, tokens, false)?;

            <Deposits<T>>::remove(&kol, &did);

            Self::deposit_event(Event::Claimed(did, kol, tokens));

            Ok(())
        }
    }

    #[pallet::genesis_config]
    pub struct GenesisConfig<T: Config> {
        pub deposit: Vec<(T::DecentralizedId, BalanceOf<T>)>,
        pub deposits: Vec<(T::DecentralizedId, T::DecentralizedId, BalanceOf<T>)>,
        pub next_class_id: T::AssetId,
    }

    #[cfg(feature = "std")]
    impl<T: Config> Default for GenesisConfig<T> {
        fn default() -> Self {
            Self {
                deposit: Default::default(),
                deposits: Default::default(),
                next_class_id: Default::default(),
            }
        }
    }

    #[pallet::genesis_build]
    impl<T: Config> GenesisBuild<T> for GenesisConfig<T> {
        fn build(&self) {
            <NextClassId<T>>::put(self.next_class_id);

            let next_class_id: u32 = self.next_class_id.try_into().unwrap_or_default();
            if next_class_id > 0 {
                for token in 0u32..next_class_id {
                    let token: T::AssetId = token.into();
                    <Date<T>>::insert(token, T::InitialMintingLockupPeriod::get());
                }
            }

            for (kol, deposit) in &self.deposit {
                <Deposit<T>>::insert(kol, deposit);
            }

            for (kol, did, deposit) in &self.deposits {
                <Deposits<T>>::insert(kol, did, deposit);
            }
        }
    }
}

impl<T: Config> Pallet<T> {
    fn try_into<S, D>(value: S) -> Result<D, DispatchError>
    where
        S: TryInto<u128>,
        D: TryFrom<u128>,
    {
        let value: u128 = value.try_into().map_err(|_| Error::<T>::Overflow)?;

        let value: D = value.try_into().map_err(|_| Error::<T>::Overflow)?;

        Ok(value)
    }
}
