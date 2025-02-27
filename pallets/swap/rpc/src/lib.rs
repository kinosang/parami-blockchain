pub use self::gen_client::Client as SwapClient;
use codec::Codec;
use jsonrpc_core::{Error as RpcError, ErrorCode, Result};
use jsonrpc_derive::rpc;
use parami_primitives::BalanceWrapper;
pub use parami_swap_rpc_runtime_api::SwapRuntimeApi;
use sp_api::ProvideRuntimeApi;
use sp_blockchain::HeaderBackend;
use sp_runtime::{
    generic::BlockId,
    traits::{Block as BlockT, MaybeDisplay, MaybeFromStr},
};
use std::sync::Arc;

#[rpc]
pub trait SwapApi<BlockHash, AssetId, Balance>
where
    Balance: MaybeDisplay + MaybeFromStr,
{
    /// Get dry-run result of add_liquidity
    ///
    /// # Arguments
    ///
    /// * `token_id` - The Asset ID
    /// * `currency` - The currency to be involved in the swap
    /// * `max_tokens` - The maximum amount of tokens to be involved in the swap
    ///
    /// # Results
    ///
    /// tuple of (tokens, liquidity)
    ///
    /// * `tokens` - The amount of tokens to be involved in the swap
    /// * `liquidity` - The amount of liquidity to be minted
    #[rpc(name = "swap_drylyAddLiquidity")]
    fn dryly_add_liquidity(
        &self,
        token_id: AssetId,
        currency: BalanceWrapper<Balance>,
        max_tokens: BalanceWrapper<Balance>,
        at: Option<BlockHash>,
    ) -> Result<(BalanceWrapper<Balance>, BalanceWrapper<Balance>)>;

    /// Get dry-run result of remove_liquidity
    ///
    /// # Arguments
    ///
    /// * `lp_token_id` - The Liquidity Provider Token ID
    ///
    /// # Results
    ///
    /// tuple of (token_id, liquidity, tokens, currency)
    ///
    /// * `token_id` - The Asset ID
    /// * `liquidity` - The amount of liquidity removed
    /// * `tokens` - The amount of tokens to be returned
    /// * `currency` - The currency to be returned
    #[rpc(name = "swap_drylyRemoveLiquidity")]
    fn dryly_remove_liquidity(
        &self,
        lp_token_id: AssetId,
        at: Option<BlockHash>,
    ) -> Result<(
        AssetId,
        BalanceWrapper<Balance>,
        BalanceWrapper<Balance>,
        BalanceWrapper<Balance>,
    )>;

    /// Get dry-run result of buy_tokens
    ///
    /// # Arguments
    ///
    /// * `token_id` - The Asset ID
    /// * `tokens` - The amount of tokens to be bought
    ///
    /// # Results
    ///
    /// The currency needed
    #[rpc(name = "swap_drylyBuyTokens")]
    fn dryly_buy_tokens(
        &self,
        token_id: AssetId,
        tokens: BalanceWrapper<Balance>,
        at: Option<BlockHash>,
    ) -> Result<BalanceWrapper<Balance>>;

    /// Get dry-run result of sell_tokens
    ///
    /// # Arguments
    ///
    /// * `token_id` - The Asset ID
    /// * `tokens` - The amount of tokens to be sold
    ///
    /// # Results
    ///
    /// The currency to be gained
    #[rpc(name = "swap_drylySellTokens")]
    fn dryly_sell_tokens(
        &self,
        token_id: AssetId,
        tokens: BalanceWrapper<Balance>,
        at: Option<BlockHash>,
    ) -> Result<BalanceWrapper<Balance>>;

    /// Get dry-run result of sell_currency
    ///
    /// # Arguments
    ///
    /// * `token_id` - The Asset ID
    /// * `currency` - The currency to be sold
    ///
    /// # Results
    ///
    /// The amount of tokens to be gained
    #[rpc(name = "swap_drylySellCurrency")]
    fn dryly_sell_currency(
        &self,
        token_id: AssetId,
        currency: BalanceWrapper<Balance>,
        at: Option<BlockHash>,
    ) -> Result<BalanceWrapper<Balance>>;

    /// Get dry-run result of buy_currency
    ///
    /// # Arguments
    ///
    /// * `token_id` - The Asset ID
    /// * `currency` - The currency to be bought
    ///
    /// # Results
    ///
    /// The amount of tokens needed
    #[rpc(name = "swap_drylyBuyCurrency")]
    fn dryly_buy_currency(
        &self,
        token_id: AssetId,
        currency: BalanceWrapper<Balance>,
        at: Option<BlockHash>,
    ) -> Result<BalanceWrapper<Balance>>;

    /// Calculate staking reward
    ///
    /// # Arguments
    ///
    /// * `lp_token_id` - The Liquidity Provider Token ID
    ///
    /// # Results
    ///
    /// The amount of reward tokens
    #[rpc(name = "swap_calculateReward")]
    fn calculate_reward(
        &self,
        lp_token_id: AssetId,
        at: Option<BlockHash>,
    ) -> Result<BalanceWrapper<Balance>>;
}

pub struct SwapsRpcHandler<C, Block, AssetId, Balance> {
    client: Arc<C>,
    _marker: std::marker::PhantomData<(Block, AssetId, Balance)>,
}

impl<C, Block, AssetId, Balance> SwapsRpcHandler<C, Block, AssetId, Balance> {
    pub fn new(client: Arc<C>) -> Self {
        Self {
            client,
            _marker: Default::default(),
        }
    }
}

impl<C, Block, AssetId, Balance> SwapApi<<Block as BlockT>::Hash, AssetId, Balance>
    for SwapsRpcHandler<C, Block, AssetId, Balance>
where
    Block: BlockT,
    C: Send + Sync + 'static + ProvideRuntimeApi<Block> + HeaderBackend<Block>,
    C::Api: SwapRuntimeApi<Block, AssetId, Balance>,
    AssetId: Codec + Send + Sync + 'static,
    Balance: Codec + MaybeDisplay + MaybeFromStr + Send + Sync + 'static,
{
    fn dryly_add_liquidity(
        &self,
        token_id: AssetId,
        currency: BalanceWrapper<Balance>,
        max_tokens: BalanceWrapper<Balance>,
        at: Option<<Block as BlockT>::Hash>,
    ) -> Result<(BalanceWrapper<Balance>, BalanceWrapper<Balance>)> {
        let api = self.client.runtime_api();
        let at = BlockId::hash(at.unwrap_or_else(|| self.client.info().best_hash));

        let res = api
            .dryly_add_liquidity(&at, token_id, currency, max_tokens)
            .map_err(|e| RpcError {
                code: ErrorCode::InternalError,
                message: "Unable to dry-run mint.".into(),
                data: Some(format!("{:?}", e).into()),
            })?;

        res.map_err(|e| RpcError {
            code: ErrorCode::ServerError(1),
            message: "Unable to dry-run mint.".into(),
            data: Some(format!("{:?}", e).into()),
        })
    }

    fn dryly_remove_liquidity(
        &self,
        lp_token_id: AssetId,
        at: Option<<Block as BlockT>::Hash>,
    ) -> Result<(
        AssetId,
        BalanceWrapper<Balance>,
        BalanceWrapper<Balance>,
        BalanceWrapper<Balance>,
    )> {
        let api = self.client.runtime_api();
        let at = BlockId::hash(at.unwrap_or_else(|| self.client.info().best_hash));

        let res = api
            .dryly_remove_liquidity(&at, lp_token_id)
            .map_err(|e| RpcError {
                code: ErrorCode::InternalError,
                message: "Unable to dry-run burn.".into(),
                data: Some(format!("{:?}", e).into()),
            })?;

        res.map_err(|e| RpcError {
            code: ErrorCode::ServerError(1),
            message: "Unable to dry-run burn.".into(),
            data: Some(format!("{:?}", e).into()),
        })
    }

    fn dryly_buy_tokens(
        &self,
        token_id: AssetId,
        tokens: BalanceWrapper<Balance>,
        at: Option<<Block as BlockT>::Hash>,
    ) -> Result<BalanceWrapper<Balance>> {
        let api = self.client.runtime_api();
        let at = BlockId::hash(at.unwrap_or_else(|| self.client.info().best_hash));

        let res = api
            .dryly_buy_tokens(&at, token_id, tokens)
            .map_err(|e| RpcError {
                code: ErrorCode::InternalError,
                message: "Unable to dry-run token_out.".into(),
                data: Some(format!("{:?}", e).into()),
            })?;

        res.map_err(|e| RpcError {
            code: ErrorCode::ServerError(1),
            message: "Unable to dry-run token_out.".into(),
            data: Some(format!("{:?}", e).into()),
        })
    }

    fn dryly_sell_tokens(
        &self,
        token_id: AssetId,
        tokens: BalanceWrapper<Balance>,
        at: Option<<Block as BlockT>::Hash>,
    ) -> Result<BalanceWrapper<Balance>> {
        let api = self.client.runtime_api();
        let at = BlockId::hash(at.unwrap_or_else(|| self.client.info().best_hash));

        let res = api
            .dryly_sell_tokens(&at, token_id, tokens)
            .map_err(|e| RpcError {
                code: ErrorCode::InternalError,
                message: "Unable to dry-run token_in.".into(),
                data: Some(format!("{:?}", e).into()),
            })?;

        res.map_err(|e| RpcError {
            code: ErrorCode::ServerError(1),
            message: "Unable to dry-run token_in.".into(),
            data: Some(format!("{:?}", e).into()),
        })
    }

    fn dryly_sell_currency(
        &self,
        token_id: AssetId,
        currency: BalanceWrapper<Balance>,
        at: Option<<Block as BlockT>::Hash>,
    ) -> Result<BalanceWrapper<Balance>> {
        let api = self.client.runtime_api();
        let at = BlockId::hash(at.unwrap_or_else(|| self.client.info().best_hash));

        let res = api
            .dryly_sell_currency(&at, token_id, currency)
            .map_err(|e| RpcError {
                code: ErrorCode::InternalError,
                message: "Unable to dry-run quote_in.".into(),
                data: Some(format!("{:?}", e).into()),
            })?;

        res.map_err(|e| RpcError {
            code: ErrorCode::ServerError(1),
            message: "Unable to dry-run quote_in.".into(),
            data: Some(format!("{:?}", e).into()),
        })
    }

    fn dryly_buy_currency(
        &self,
        token_id: AssetId,
        currency: BalanceWrapper<Balance>,
        at: Option<<Block as BlockT>::Hash>,
    ) -> Result<BalanceWrapper<Balance>> {
        let api = self.client.runtime_api();
        let at = BlockId::hash(at.unwrap_or_else(|| self.client.info().best_hash));

        let res = api
            .dryly_buy_currency(&at, token_id, currency)
            .map_err(|e| RpcError {
                code: ErrorCode::InternalError,
                message: "Unable to dry-run quote_out.".into(),
                data: Some(format!("{:?}", e).into()),
            })?;

        res.map_err(|e| RpcError {
            code: ErrorCode::ServerError(1),
            message: "Unable to dry-run quote_out.".into(),
            data: Some(format!("{:?}", e).into()),
        })
    }

    fn calculate_reward(
        &self,
        lp_token_id: AssetId,
        at: Option<<Block as BlockT>::Hash>,
    ) -> Result<BalanceWrapper<Balance>> {
        let api = self.client.runtime_api();
        let at = BlockId::hash(at.unwrap_or_else(|| self.client.info().best_hash));

        let res = api
            .calculate_reward(&at, lp_token_id)
            .map_err(|e| RpcError {
                code: ErrorCode::InternalError,
                message: "Unable to calculate reward.".into(),
                data: Some(format!("{:?}", e).into()),
            })?;

        res.map_err(|e| RpcError {
            code: ErrorCode::ServerError(1),
            message: "Unable to calculate reward.".into(),
            data: Some(format!("{:?}", e).into()),
        })
    }
}
