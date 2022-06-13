use codec::{Decode, Encode};
use scale_info::TypeInfo;
#[cfg(feature = "std")]
use serde::{Deserialize, Serialize};
use sp_runtime::RuntimeDebug;
use sp_std::prelude::*;

#[derive(Clone, Decode, Default, Encode, Eq, PartialEq, RuntimeDebug, TypeInfo)]
#[cfg_attr(feature = "std", derive(Serialize, Deserialize))]
pub struct Metadata<A, B, D, H, N> {
    pub id: H,
    pub creator: D,
    pub pot: A,
    // TODO: migration
    pub metadata: Vec<u8>,
    pub reward_rate: u16,
    pub created: N,
    pub payout_base: B,
    pub payout_min: B,
    pub payout_max: B,
}

#[derive(Clone, Decode, Default, Encode, Eq, PartialEq, RuntimeDebug, TypeInfo)]
#[cfg_attr(feature = "std", derive(Serialize, Deserialize))]
pub struct Slot<Hash, Height, NftId, TokenId> {
    pub ad_id: Hash,
    pub nft_id: NftId,
    pub fungible_id: Option<TokenId>,
    // TODO: migration
    pub created: Height,
}
