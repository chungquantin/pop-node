#![feature(custom_test_frameworks)]
#![feature(test)]
#![test_runner(runner)]

extern crate test;

use horde::Zombienet;
use std::time::Duration;
use test::TestDescAndFn;

mod polkadot;
mod pop;

#[cfg(test)]
#[tokio::main]
pub async fn runner(tests: &[&TestDescAndFn]) {
    // Configure zombienet requirements
    let mut zombienet = Zombienet::new("./network.toml").unwrap();
    zombienet.with_relay_chain("v1.3.0").unwrap();
    // Spawn network and run integration tests
    horde::test_runner(zombienet, Duration::from_secs(30), tests)
        .await
        .unwrap()
}

#[cfg(test)]
mod tests {
    use super::polkadot::api::session::events::NewSession;
    use rand::random;
    use std::{error::Error, time::Duration};
    use subxt::{events::StaticEvent, Config, OnlineClient, PolkadotConfig};
    use subxt_signer::sr25519::{dev, Keypair};
    use tokio::time::sleep;

    const RELAY_BLOCK: Duration = Duration::from_secs(6);
    const BLOCK: Duration = Duration::from_secs(12);

    #[tokio::test]
    async fn reserve_transfer() -> Result<(), Box<dyn Error>> {
        // Wait for parachain to be onboarded
        let relay = OnlineClient::<PolkadotConfig>::from_url("ws://127.0.0.1:9944").await?;
        relay
            .wait_for_event::<NewSession>(|_e| true, 15 * RELAY_BLOCK)
            .await?;

        const AMOUNT: u128 = 1_000_000_000_000;
        let beneficiary = Keypair::from_seed(random::<[u8; 32]>())?;

        // Reserve transfer relay chain token to Pop
        relay
            .execute_with(|| async {
                use super::polkadot::api::{
                    self,
                    runtime_types::xcm::v3::junctions::Junctions,
                    runtime_types::{
                        staging_xcm::v3::multilocation::MultiLocation,
                        xcm::{
                            v3::{
                                junction::Junction::{AccountId32, Parachain},
                                junctions::Junctions::X1,
                                multiasset::{AssetId, Fungibility, MultiAsset, MultiAssets},
                            },
                            VersionedMultiAssets, VersionedMultiLocation,
                        },
                    },
                };

                let source = dev::alice();
                let dest = VersionedMultiLocation::V3(MultiLocation {
                    parents: 0,
                    interior: X1(Parachain(909)),
                });
                let beneficiary = VersionedMultiLocation::V3(MultiLocation {
                    parents: 0,
                    interior: X1(AccountId32 {
                        network: None,
                        id: beneficiary.public_key().0,
                    }),
                });
                let assets = VersionedMultiAssets::V3(MultiAssets(vec![MultiAsset {
                    id: AssetId::Concrete(MultiLocation {
                        parents: 0,
                        interior: Junctions::Here,
                    }),
                    fun: Fungibility::Fungible(AMOUNT),
                }]));
                let reserve_transfer =
                    api::tx()
                        .xcm_pallet()
                        .reserve_transfer_assets(dest, beneficiary, assets, 0);
                relay
                    .tx()
                    .sign_and_submit_then_watch_default(&reserve_transfer, &source)
                    .await?
                    .wait_for_finalized_success()
                    .await
            })
            .await?;

        // Transfer back to relay chain
        let pop = OnlineClient::<PolkadotConfig>::from_url("ws://127.0.0.1:9945").await?;
        pop.execute_with(|| async {
            use super::pop::api::{
                self,
                balances::events::Endowed,
                runtime_types::{
                    staging_xcm::v3::multilocation::MultiLocation,
                    xcm::{
                        v3::{
                            junction::Junction::AccountId32,
                            junctions::{Junctions::Here, Junctions::X1},
                            multiasset::{
                                AssetId, Fungibility, MultiAsset, MultiAssetFilter, MultiAssets,
                                WildMultiAsset,
                            },
                            Instruction::{
                                BuyExecution, DepositAsset, InitiateReserveWithdraw, WithdrawAsset,
                            },
                            WeightLimit, Xcm,
                        },
                        VersionedMultiLocation, VersionedXcm,
                    },
                },
            };

            pop.wait_for_event::<Endowed>(
                |e| {
                    e.account == beneficiary.public_key().to_account_id()
                        && e.free_balance > 0
                        && e.free_balance < AMOUNT
                },
                5 * BLOCK,
            )
            .await?;

            let dest = VersionedMultiLocation::V3(MultiLocation {
                parents: 1,
                interior: Here,
            });

            let source = beneficiary;

            // Todo: needs to be implemented as a dispatchable due to origin prepending
            let message = VersionedXcm::V3(Xcm(vec![
                WithdrawAsset(MultiAssets(vec![MultiAsset {
                    id: AssetId::Concrete(MultiLocation {
                        parents: 0,
                        interior: Here,
                    }),
                    fun: Fungibility::Fungible(AMOUNT / 2),
                }])),
                // https://substrate.stackexchange.com/a/3036/3138
                InitiateReserveWithdraw {
                    assets: MultiAssetFilter::Wild(WildMultiAsset::All),
                    reserve: MultiLocation {
                        parents: 0,
                        interior: Here,
                    },
                    xcm: Xcm(vec![
                        BuyExecution {
                            fees: MultiAsset {
                                id: AssetId::Concrete(MultiLocation {
                                    parents: 0,
                                    interior: Here,
                                }),
                                fun: Fungibility::Fungible(AMOUNT / 10),
                            },
                            weight_limit: WeightLimit::Unlimited,
                        },
                        DepositAsset {
                            assets: MultiAssetFilter::Wild(WildMultiAsset::AllCounted(1)),
                            beneficiary: MultiLocation {
                                parents: 0,
                                interior: X1(AccountId32 {
                                    network: None,
                                    id: source.public_key().0,
                                }),
                            },
                        },
                    ]),
                },
            ]));
            let reserve_transfer = api::tx().polkadot_xcm().send(dest, message);
            pop.tx()
                .sign_and_submit_then_watch_default(&reserve_transfer, &source)
                .await?
                .wait_for_finalized_success()
                .await
                .map_err(|e| Box::new(e) as Box<dyn Error>)
        })
        .await?;

        // Todo: await successful event on relay chain denoting user account being funded as a result of transfer

        Ok(())
    }

    #[async_trait::async_trait]
    trait OnlineClientExt<T: Config> {
        fn execute_with<R>(&self, execute: impl FnOnce() -> R) -> R;
        async fn wait_for_event<E: StaticEvent>(
            &self,
            eval: impl Fn(E) -> bool + Send + Sync,
            timeout: Duration,
        ) -> Result<(), Box<dyn Error>>;
    }

    #[async_trait::async_trait]
    impl<T: Config> OnlineClientExt<T> for OnlineClient<T> {
        fn execute_with<R>(&self, execute: impl FnOnce() -> R) -> R {
            execute()
        }
        async fn wait_for_event<E: StaticEvent>(
            &self,
            eval: impl Fn(E) -> bool + Send + Sync,
            timeout: Duration,
        ) -> Result<(), Box<dyn Error>> {
            tokio::time::timeout(timeout, async {
                loop {
                    let events = self.events().at_latest().await?;
                    if let Some(endowed) = events.find_first::<E>()? {
                        if eval(endowed) {
                            return Ok::<(), Box<dyn Error>>(());
                        }
                    }

                    sleep(Duration::from_secs(1)).await;
                }
            })
            .await?
        }
    }
}
