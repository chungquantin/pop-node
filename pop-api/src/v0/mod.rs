use crate::{
	primitives::storage_keys::{ParachainSystemKeys, RuntimeStateKeys},
	BlockNumber, StatusCode,
};

pub mod assets;
pub mod balances;
pub mod cross_chain;
pub mod nfts;
pub mod state;

pub fn relay_chain_block_number() -> Result<BlockNumber, StatusCode> {
	state::read(RuntimeStateKeys::ParachainSystem(ParachainSystemKeys::LastRelayChainBlockNumber))
}

#[derive(scale::Encode)]
pub(crate) enum RuntimeCall {
	#[codec(index = 10)]
	Balances(balances::BalancesCall),
	#[codec(index = 50)]
	Nfts(nfts::NftCalls),
	#[codec(index = 52)]
	Assets(assets::pallets::assets::AssetsCall),
}
