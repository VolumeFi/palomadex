use cosmwasm_std::{Addr, Api, CustomMsg, CustomQuery, Storage};
use cw_multi_test::{
    AppResponse, Bank, ContractWrapper, Distribution, Executor, Gov, Ibc, Module, Staking, Stargate,
};
use palomadex::native_coin_registry::{ExecuteMsg, InstantiateMsg};
use serde::de::DeserializeOwned;

use crate::{palomadex_address, WKApp, PALOMADEX};

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
    use palomadex_native_coin_registry as cnt;
    let contract = Box::new(ContractWrapper::new_with_empty(
        cnt::contract::execute,
        cnt::contract::instantiate,
        cnt::contract::query,
    ));

    app.borrow_mut().store_code(contract)
}

pub struct MockCoinRegistryBuilder<B, A, S, C: Module, X, D, I, G, T> {
    pub app: WKApp<B, A, S, C, X, D, I, G, T>,
}

impl<B, A, S, C, X, D, I, G, T> MockCoinRegistryBuilder<B, A, S, C, X, D, I, G, T>
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
    pub fn instantiate(self) -> MockCoinRegistry<B, A, S, C, X, D, I, G, T> {
        let code_id = store_code(&self.app);
        let palomadex = palomadex_address();

        let address = self
            .app
            .borrow_mut()
            .instantiate_contract(
                code_id,
                palomadex.clone(),
                &InstantiateMsg {
                    owner: PALOMADEX.to_owned(),
                },
                &[],
                "Palomadex Coin Registry",
                Some(PALOMADEX.to_owned()),
            )
            .unwrap();

        self.app
            .borrow_mut()
            .execute_contract(
                palomadex,
                address.clone(),
                &ExecuteMsg::Add {
                    native_coins: vec![("ustake".to_owned(), 6), ("ucosmos".to_owned(), 6)],
                },
                &[],
            )
            .unwrap();

        MockCoinRegistry {
            app: self.app,
            address,
        }
    }
}

pub struct MockCoinRegistry<B, A, S, C: Module, X, D, I, G, T> {
    pub app: WKApp<B, A, S, C, X, D, I, G, T>,
    pub address: Addr,
}

impl<B, A, S, C, X, D, I, G, T> MockCoinRegistry<B, A, S, C, X, D, I, G, T>
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
    pub fn add(&self, coins: Vec<(String, u8)>) -> AppResponse {
        let palomadex = palomadex_address();

        self.app
            .borrow_mut()
            .execute_contract(
                palomadex,
                self.address.clone(),
                &ExecuteMsg::Add {
                    native_coins: coins,
                },
                &[],
            )
            .unwrap()
    }
}
