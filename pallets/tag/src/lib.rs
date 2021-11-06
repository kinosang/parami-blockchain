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

mod types;

use frame_support::{
    dispatch::DispatchResult,
    ensure,
    traits::{Currency, ExistenceRequirement::KeepAlive, StoredMap, WithdrawReasons},
    StorageHasher,
};
use scale_info::TypeInfo;
use sp_runtime::{
    traits::{Hash, MaybeSerializeDeserialize, Member},
    DispatchError,
};
use sp_std::prelude::*;

use weights::WeightInfo;

type AccountOf<T> = <T as frame_system::Config>::AccountId;
type BalanceOf<T> = <<T as Config>::Currency as Currency<AccountOf<T>>>::Balance;
type HashOf<T> = <<T as frame_system::Config>::Hashing as Hash>::Output;
type HeightOf<T> = <T as frame_system::Config>::BlockNumber;
type MetaOf<T> = types::Metadata<<T as Config>::DecentralizedId, HeightOf<T>>;

#[frame_support::pallet]
pub mod pallet {
    use super::*;
    use frame_support::pallet_prelude::*;
    use frame_system::pallet_prelude::*;

    #[pallet::config]
    pub trait Config: frame_system::Config {
        type Event: From<Event<Self>> + IsType<<Self as frame_system::Config>::Event>;

        /// The currency trait
        type Currency: Currency<Self::AccountId>;

        /// The DID type
        type DecentralizedId: Parameter
            + Member
            + MaybeSerializeDeserialize
            + Ord
            + Default
            + Copy
            + sp_std::hash::Hash
            + AsRef<[u8]>
            + AsMut<[u8]>
            + MaxEncodedLen
            + TypeInfo;

        /// The hashing algorithm being used for hash map to hash tags
        type Hashing: StorageHasher;

        /// Submission fee to create new tags
        #[pallet::constant]
        type SubmissionFee: Get<BalanceOf<Self>>;

        /// The origin which may do calls
        type CallOrigin: EnsureOrigin<
            Self::Origin,
            Success = (Self::DecentralizedId, Self::AccountId),
        >;

        /// The origin which may forcibly create tag or otherwise alter privileged attributes
        type ForceOrigin: EnsureOrigin<Self::Origin>;

        /// Weight information for extrinsics in this pallet.
        type WeightInfo: WeightInfo;
    }

    #[pallet::pallet]
    #[pallet::generate_store(pub(super) trait Store)]
    pub struct Pallet<T>(PhantomData<T>);

    #[pallet::storage]
    #[pallet::getter(fn meta)]
    pub(super) type Metadata<T: Config> = StorageMap<_, <T as Config>::Hashing, Vec<u8>, MetaOf<T>>;

    /// Tags of an advertisement
    #[pallet::storage]
    #[pallet::getter(fn tags_of)]
    pub(super) type TagsOf<T: Config> = StorageMap<_, Identity, HashOf<T>, Vec<Vec<u8>>>;

    /// Tags and Scores of a DID
    #[pallet::storage]
    #[pallet::getter(fn personas_of)]
    pub(super) type PersonasOf<T: Config> =
        StorageDoubleMap<_, Identity, T::DecentralizedId, Identity, Vec<u8>, i64>;

    /// Tags and Scores of a KOL
    #[pallet::storage]
    #[pallet::getter(fn influences_of)]
    pub(super) type InfluencesOf<T: Config> =
        StorageDoubleMap<_, Identity, T::DecentralizedId, Identity, Vec<u8>, i64>;

    #[pallet::event]
    #[pallet::generate_deposit(pub(super) fn deposit_event)]
    pub enum Event<T: Config> {
        /// Tag created \[hash, creator\]
        Created(Vec<u8>, T::DecentralizedId),
    }

    #[pallet::hooks]
    impl<T: Config> Hooks<BlockNumberFor<T>> for Pallet<T> {}

    #[pallet::error]
    pub enum Error<T> {
        Exists,
        InsufficientBalance,
        NotExists,
    }

    #[pallet::call]
    impl<T: Config> Pallet<T> {
        #[pallet::weight(T::WeightInfo::create(tag.len() as u32))]
        pub fn create(origin: OriginFor<T>, tag: Vec<u8>) -> DispatchResult {
            let (did, who) = T::CallOrigin::ensure_origin(origin)?;

            ensure!(!<Metadata<T>>::contains_key(&tag), Error::<T>::Exists);

            let fee = T::SubmissionFee::get();

            let imb = T::Currency::burn(fee);

            let res = T::Currency::settle(&who, imb, WithdrawReasons::FEE, KeepAlive);

            ensure!(res.is_ok(), Error::<T>::InsufficientBalance);

            let hash = Self::inner_create(did, tag);

            Self::deposit_event(Event::Created(hash, did));

            Ok(())
        }

        #[pallet::weight(T::WeightInfo::force_create(tag.len() as u32))]
        pub fn force_create(origin: OriginFor<T>, tag: Vec<u8>) -> DispatchResult {
            T::ForceOrigin::ensure_origin(origin)?;

            ensure!(!<Metadata<T>>::contains_key(&tag), Error::<T>::Exists);

            let did = T::DecentralizedId::default();

            let hash = Self::inner_create(did, tag);

            Self::deposit_event(Event::Created(hash, did));

            Ok(())
        }
    }

    #[pallet::genesis_config]
    pub struct GenesisConfig<T> {
        pub tags: Vec<Vec<u8>>,
        pub phantom: PhantomData<T>,
    }

    #[cfg(feature = "std")]
    impl<T: Config> Default for GenesisConfig<T> {
        fn default() -> Self {
            Self {
                tags: Default::default(),
                phantom: Default::default(),
            }
        }
    }

    #[pallet::genesis_build]
    impl<T: Config> GenesisBuild<T> for GenesisConfig<T> {
        fn build(&self) {
            for tag in &self.tags {
                <Metadata<T>>::insert(
                    tag,
                    types::Metadata {
                        creator: T::DecentralizedId::default(),
                        created: Default::default(),
                    },
                );
            }
        }
    }
}

impl<T: Config> Pallet<T> {
    pub fn key<K: AsRef<Vec<u8>>>(tag: K) -> Vec<u8> {
        <Metadata<T>>::hashed_key_for(tag.as_ref())
    }

    fn inner_create(creator: T::DecentralizedId, tag: Vec<u8>) -> Vec<u8> {
        let created = <frame_system::Pallet<T>>::block_number();

        <Metadata<T>>::insert(&tag, types::Metadata { creator, created });

        Self::key(&tag)
    }

    /// update score of a tag for a DID
    pub fn influence(did: T::DecentralizedId, tag: Vec<u8>, delta: i64) -> DispatchResult {
        ensure!(<Metadata<T>>::contains_key(&tag), Error::<T>::NotExists);

        let hash = Self::key(&tag);

        <PersonasOf<T>>::mutate(&did, hash, |maybe| {
            if let Some(score) = maybe {
                *score += delta;
            } else {
                *maybe = Some(delta);
            }
        });

        Ok(())
    }

    /// update score of a tag for a KOL
    pub fn impact(did: T::DecentralizedId, tag: Vec<u8>, delta: i64) -> DispatchResult {
        ensure!(<Metadata<T>>::contains_key(&tag), Error::<T>::NotExists);

        let hash = Self::key(&tag);

        <InfluencesOf<T>>::mutate(&did, hash, |maybe| {
            if let Some(score) = maybe {
                *score += delta;
            } else {
                *maybe = Some(delta);
            }
        });

        Ok(())
    }
}

impl<T: Config> StoredMap<Vec<u8>, Vec<u8>> for Pallet<T> {
    fn get(k: &Vec<u8>) -> Vec<u8> {
        if <Metadata<T>>::contains_key(k) {
            Self::key(k)
        } else {
            Default::default()
        }
    }

    fn try_mutate_exists<R, E: From<DispatchError>>(
        _k: &Vec<u8>,
        f: impl FnOnce(&mut Option<Vec<u8>>) -> Result<R, E>,
    ) -> Result<R, E> {
        let mut some = None;
        f(&mut some)
    }
}

impl<T: Config> StoredMap<HashOf<T>, Vec<Vec<u8>>> for Pallet<T> {
    fn get(k: &HashOf<T>) -> Vec<Vec<u8>> {
        match <TagsOf<T>>::get(k) {
            Some(tags) => tags,
            None => Default::default(),
        }
    }

    fn try_mutate_exists<R, E: From<DispatchError>>(
        k: &HashOf<T>,
        f: impl FnOnce(&mut Option<Vec<Vec<u8>>>) -> Result<R, E>,
    ) -> Result<R, E> {
        <TagsOf<T>>::mutate(k, f)
    }
}
