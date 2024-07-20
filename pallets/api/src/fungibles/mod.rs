#![cfg_attr(not(feature = "std"), no_std)]

pub use pallet::*;

#[cfg(test)]
mod tests;

#[frame_support::pallet]
pub mod pallet {
	use frame_support::{
		dispatch::{DispatchResult, DispatchResultWithPostInfo, WithPostDispatchInfo},
		pallet_prelude::*,
		traits::fungibles::Inspect,
	};
	use frame_system::pallet_prelude::*;
	use pallet_assets::WeightInfo;
	use sp_runtime::traits::StaticLookup;

	type AccountIdOf<T> = <T as frame_system::Config>::AccountId;
	type AssetIdOf<T> = <pallet_assets::Pallet<T, AssetsInstanceOf<T>> as Inspect<
		<T as frame_system::Config>::AccountId,
	>>::AssetId;
	type AssetIdParameterOf<T> =
		<T as pallet_assets::Config<AssetsInstanceOf<T>>>::AssetIdParameter;
	type Assets<T> = pallet_assets::Pallet<T, AssetsInstanceOf<T>>;
	type AssetsInstanceOf<T> = <T as Config>::AssetsInstance;
	type AssetsWeightInfo<T> = <T as pallet_assets::Config<AssetsInstanceOf<T>>>::WeightInfo;
	type BalanceOf<T> = <pallet_assets::Pallet<T, AssetsInstanceOf<T>> as Inspect<
		<T as frame_system::Config>::AccountId,
	>>::Balance;

	/// The required input for state queries in pallet assets.
	#[derive(Encode, Decode, Debug, MaxEncodedLen)]
	pub enum AssetsKeys<T: Config> {
		#[codec(index = 0)]
		TotalSupply(AssetIdOf<T>),
		#[codec(index = 1)]
		BalanceOf(AssetIdOf<T>, AccountIdOf<T>),
		#[codec(index = 2)]
		Allowance(AssetIdOf<T>, AccountIdOf<T>, AccountIdOf<T>),
		#[codec(index = 3)]
		TokenName(AssetIdOf<T>),
		#[codec(index = 4)]
		TokenSymbol(AssetIdOf<T>),
		#[codec(index = 5)]
		TokenDecimals(AssetIdOf<T>),
	}

	#[pallet::config]
	pub trait Config: frame_system::Config + pallet_assets::Config<Self::AssetsInstance> {
		type AssetsInstance;
	}

	#[pallet::pallet]
	pub struct Pallet<T>(_);

	#[pallet::call]
	impl<T: Config> Pallet<T> {
		#[pallet::call_index(9)]
		#[pallet::weight(AssetsWeightInfo::<T>::transfer_keep_alive())]
		pub fn transfer(
			origin: OriginFor<T>,
			id: AssetIdOf<T>,
			target: AccountIdOf<T>,
			amount: BalanceOf<T>,
		) -> DispatchResult {
			let target = T::Lookup::unlookup(target);
			Assets::<T>::transfer_keep_alive(origin, id.into(), target, amount)
		}

		#[pallet::call_index(10)]
		#[pallet::weight(AssetsWeightInfo::<T>::cancel_approval() + AssetsWeightInfo::<T>::approve_transfer())]
		pub fn approve(
			origin: OriginFor<T>,
			id: AssetIdOf<T>,
			spender: AccountIdOf<T>,
			value: BalanceOf<T>,
		) -> DispatchResultWithPostInfo {
			let spender = T::Lookup::unlookup(spender);
			let id: AssetIdParameterOf<T> = id.into();
			Assets::<T>::cancel_approval(origin.clone(), id.clone(), spender.clone())
				.map_err(|e| e.with_weight(AssetsWeightInfo::<T>::cancel_approval()))?;
			Assets::<T>::approve_transfer(origin, id, spender, value).map_err(|e| {
				e.with_weight(
					AssetsWeightInfo::<T>::cancel_approval()
						+ AssetsWeightInfo::<T>::approve_transfer(),
				)
			})?;
			Ok(().into())
		}
	}

	impl<T: Config> Pallet<T> {
		pub fn total_supply(id: AssetIdOf<T>) -> BalanceOf<T> {
			Assets::<T>::total_supply(id)
		}
		pub fn balance_of(id: AssetIdOf<T>, owner: &AccountIdOf<T>) -> BalanceOf<T> {
			Assets::<T>::balance(id, owner)
		}
	}
}
