use anyhow::Result as AnyResult;

use cosmwasm_std::{to_json_binary, Addr, Api, CustomMsg, CustomQuery, Storage};
use cw_multi_test::{
    AppResponse, Bank, ContractWrapper, Distribution, Executor, Gov, Ibc, Module, Staking, Stargate,
};
use palomadex::{
    asset::{AssetInfo, PairInfo},
    factory::{ConfigResponse, ExecuteMsg, InstantiateMsg, PairConfig, PairType, QueryMsg},
    pair::StablePoolParams,
};
use serde::de::DeserializeOwned;

use crate::{
    palomadex_address, MockCoinRegistry, MockCoinRegistryBuilder, MockStablePair, MockXykPair,
    WKApp, PALOMADEX,
};

pub fn store_code<B, A, S, C, X, D, I, G, T>(app: &WKApp<B, A, S, C, X, D, I, G, T>) -> u64
where
    B: Bank,
    A: Api,
    S: Storage,
    C: Module,
    X: Staking,
    D: Distribution,
    I: Ibc,
    G: Gov,
    T: Stargate,
    C::ExecT: CustomMsg + DeserializeOwned + 'static,
    C::QueryT: CustomQuery + DeserializeOwned + 'static,
{
    use palomadex_factory as cnt;
    let contract = Box::new(
        ContractWrapper::new_with_empty(
            cnt::contract::execute,
            cnt::contract::instantiate,
            cnt::contract::query,
        )
        .with_reply_empty(cnt::contract::reply),
    );

    app.borrow_mut().store_code(contract)
}

pub struct MockFactoryBuilder<B, A, S, C: Module, X, D, I, G, T> {
    pub app: WKApp<B, A, S, C, X, D, I, G, T>,
}

impl<B, A, S, C, X, D, I, G, T> MockFactoryBuilder<B, A, S, C, X, D, I, G, T>
where
    B: Bank,
    A: Api,
    S: Storage,
    C: Module,
    X: Staking,
    D: Distribution,
    I: Ibc,
    G: Gov,
    T: Stargate,
    C::ExecT: CustomMsg + DeserializeOwned + 'static,
    C::QueryT: CustomQuery + DeserializeOwned + 'static,
{
    pub fn new(app: &WKApp<B, A, S, C, X, D, I, G, T>) -> Self {
        Self { app: app.clone() }
    }

    pub fn instantiate(self) -> MockFactory<B, A, S, C, X, D, I, G, T> {
        let code_id = store_code(&self.app);
        let palomadex = palomadex_address();

        let xyk_code_id = crate::pair::store_code(&self.app);
        let stable_code_id = crate::pair_stable::store_code(&self.app);

        let pair_configs = vec![
            PairConfig {
                code_id: xyk_code_id,
                pair_type: PairType::Xyk {},
                is_disabled: false,
                is_generator_disabled: false,
                total_fee_bps: 30,
                maker_fee_bps: 3333,
                permissioned: false,
            },
            PairConfig {
                code_id: stable_code_id,
                pair_type: PairType::Stable {},
                is_disabled: false,
                is_generator_disabled: false,
                total_fee_bps: 5,
                maker_fee_bps: 5000,
                permissioned: false,
            },
        ];

        let token_code_id = crate::token::store_code(&self.app);
        let whitelist_code_id = crate::whitelist::store_code(&self.app);

        let coin_registry = MockCoinRegistryBuilder::new(&self.app).instantiate();

        let address = self
            .app
            .borrow_mut()
            .instantiate_contract(
                code_id,
                palomadex,
                &InstantiateMsg {
                    owner: PALOMADEX.to_owned(),
                    fee_address: None,
                    pair_configs,
                    token_code_id,
                    generator_address: None,
                    whitelist_code_id,
                    coin_registry_address: coin_registry.address.to_string(),
                },
                &[],
                "Palomadex Factory",
                Some(PALOMADEX.to_owned()),
            )
            .unwrap();

        MockFactory {
            app: self.app,
            address,
        }
    }
}

pub struct MockFactory<B, A, S, C: Module, X, D, I, G, T> {
    pub app: WKApp<B, A, S, C, X, D, I, G, T>,
    pub address: Addr,
}

pub type MockFactoryOpt<B, A, S, C, X, D, I, G, T> = Option<MockFactory<B, A, S, C, X, D, I, G, T>>;

impl<B, A, S, C, X, D, I, G, T> MockFactory<B, A, S, C, X, D, I, G, T>
where
    B: Bank,
    A: Api,
    S: Storage,
    C: Module,
    X: Staking,
    D: Distribution,
    I: Ibc,
    G: Gov,
    T: Stargate,
    C::ExecT: CustomMsg + DeserializeOwned + 'static,
    C::QueryT: CustomQuery + DeserializeOwned + 'static,
{
    pub fn whitelist_code_id(&self) -> u64 {
        let config: ConfigResponse = self
            .app
            .borrow()
            .wrap()
            .query_wasm_smart(self.address.clone(), &QueryMsg::Config {})
            .unwrap();

        config.whitelist_code_id
    }

    pub fn token_code_id(&self) -> u64 {
        let config: ConfigResponse = self
            .app
            .borrow()
            .wrap()
            .query_wasm_smart(self.address.clone(), &QueryMsg::Config {})
            .unwrap();

        config.token_code_id
    }

    pub fn instantiate_xyk_pair(
        &self,
        asset_infos: &[AssetInfo],
    ) -> MockXykPair<B, A, S, C, X, D, I, G, T> {
        let palomadex = palomadex_address();

        self.app
            .borrow_mut()
            .execute_contract(
                palomadex,
                self.address.clone(),
                &ExecuteMsg::CreatePair {
                    pair_type: PairType::Xyk {},
                    asset_infos: asset_infos.to_vec(),
                    init_params: None,
                },
                &[],
            )
            .unwrap();

        let res: PairInfo = self
            .app
            .borrow()
            .wrap()
            .query_wasm_smart(
                &self.address,
                &QueryMsg::Pair {
                    asset_infos: asset_infos.to_vec(),
                },
            )
            .unwrap();

        MockXykPair {
            app: self.app.clone(),
            address: res.contract_addr,
        }
    }

    /// Set init_params to None to use the defaults
    pub fn instantiate_stable_pair(
        &self,
        asset_infos: &[AssetInfo],
        init_params: Option<&StablePoolParams>,
    ) -> MockStablePair<B, A, S, C, X, D, I, G, T> {
        let palomadex = palomadex_address();

        let default_params = StablePoolParams {
            amp: 100,
            owner: Some(palomadex.to_string()),
        };

        self.app
            .borrow_mut()
            .execute_contract(
                palomadex,
                self.address.clone(),
                &ExecuteMsg::CreatePair {
                    pair_type: PairType::Stable {},
                    asset_infos: asset_infos.to_vec(),
                    init_params: Some(
                        to_json_binary(init_params.unwrap_or(&default_params)).unwrap(),
                    ),
                },
                &[],
            )
            .unwrap();

        let res: PairInfo = self
            .app
            .borrow()
            .wrap()
            .query_wasm_smart(
                &self.address,
                &QueryMsg::Pair {
                    asset_infos: asset_infos.to_vec(),
                },
            )
            .unwrap();

        MockStablePair {
            app: self.app.clone(),
            address: res.contract_addr,
        }
    }

    pub fn coin_registry(&self) -> MockCoinRegistry<B, A, S, C, X, D, I, G, T> {
        let config: ConfigResponse = self
            .app
            .borrow()
            .wrap()
            .query_wasm_smart(self.address.clone(), &QueryMsg::Config {})
            .unwrap();

        MockCoinRegistry {
            app: self.app.clone(),
            address: config.coin_registry_address,
        }
    }

    pub fn deregister_pair(&self, asset_infos: &[AssetInfo]) -> AnyResult<AppResponse> {
        let palomadex = palomadex_address();

        self.app.borrow_mut().execute_contract(
            palomadex,
            self.address.clone(),
            &ExecuteMsg::Deregister {
                asset_infos: asset_infos.to_vec(),
            },
            &[],
        )
    }

    pub fn config(&self) -> ConfigResponse {
        let config: ConfigResponse = self
            .app
            .borrow()
            .wrap()
            .query_wasm_smart(self.address.clone(), &QueryMsg::Config {})
            .unwrap();

        config
    }
}
