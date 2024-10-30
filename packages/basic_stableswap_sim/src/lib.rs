use cosmwasm_std::Uint256;

const DEFAULT_POOL_TOKENS: u128 = 0;
const DEFAULT_TARGET_PRICE: u128 = 1000000000000000000;
pub const MODEL_FEE_NUMERATOR: u128 = 1;
pub const MODEL_FEE_DENOMINATOR: u128 = 1000;

pub struct StableSwapModel {
    pub amp_factor: u128,
    pub balances: Vec<u128>,
    pub n_coins: u8,
    pub fee: u128,
    pub target_prices: Vec<u128>,
    pub pool_tokens: u128,
}

impl StableSwapModel {
    pub fn new(amp_factor: u128, balances: Vec<u128>, n_coins: u8) -> StableSwapModel {
        Self {
            amp_factor,
            balances,
            n_coins,
            fee: 0u128,
            target_prices: vec![DEFAULT_TARGET_PRICE; n_coins as usize],
            pool_tokens: DEFAULT_POOL_TOKENS,
        }
    }

    pub fn new_with_pool_tokens(
        amp_factor: u128,
        balances: Vec<u128>,
        n_coins: u8,
        pool_token_amount: u128,
    ) -> StableSwapModel {
        Self {
            amp_factor,
            balances,
            n_coins,
            fee: 0u128,
            target_prices: vec![DEFAULT_TARGET_PRICE; n_coins as usize],
            pool_tokens: pool_token_amount,
        }
    }

    fn xp(&self) -> Vec<u128> {
        self.balances
            .iter()
            .zip(&self.target_prices)
            .map(|(x, p)| x * p / 10u128.pow(18))
            .collect()
    }

    // pub fn sim_d(&self) -> u128 {
    //     let gil = Python::acquire_gil();
    //     return self
    //         .call0(gil.python(), "D")
    //         .unwrap()
    //         .extract(gil.python())
    //         .unwrap();
    // }

    pub fn sim_d(&self) -> u128 {
        self.d()
    }

    fn d(&self) -> u128 {
        let mut d_prev = 0;
        let xp = self.xp();
        let s: u128 = xp.iter().sum();
        let mut d = s;
        let ann = self.amp_factor * self.n_coins as u128;

        let mut counter = 0;

        while d.abs_diff(d_prev) > 1 {
            let mut d_p = d;
            for x in &xp {
                // println!("d_p = {}", d_p);
                // println!("d = {}", d);
                // println!("self.n_coins = {}", self.n_coins);
                // println!("x = {}", x);
                d_p = (Uint256::from_u128(d_p) * Uint256::from_u128(d)
                    / Uint256::from_u128(self.n_coins as u128 * x))
                .to_string()
                .parse::<u128>()
                .unwrap();
            }
            d_prev = d;
            // println!("d_p = {}", d_p);
            // println!("d = {}", d);
            // println!("self.n_coins = {}", self.n_coins);
            // println!("s = {}", s);
            // println!("ann = {}", ann);
            // println!("{}", ann * s + d_p * self.n_coins as u128);

            d = (Uint256::from_u128(d) * Uint256::from_u128(ann * s + d_p * self.n_coins as u128)
                / Uint256::from_u128((ann - 1) * d + (self.n_coins as u128 + 1) * d_p))
            .to_string()
            .parse::<u128>()
            .unwrap();

            counter += 1;
            if counter > 1000 {
                break;
            }
        }

        d
    }

    fn y(&self, i: usize, j: usize, x: u128) -> u128 {
        let d: u128 = self.d();
        let mut xx = self.xp();
        xx[i] = x;
        xx.remove(j);
        let ann = self.amp_factor * self.n_coins as u128;
        let mut c = d;
        for y in &xx {
            c = c * d / (y * self.n_coins as u128);
        }
        c = c * d / (self.n_coins as u128 * ann);
        let b: i128 = xx.iter().sum::<u128>() as i128 + (d / ann) as i128 - d as i128;
        let mut y_prev = 0;
        let mut y = d;

        let mut counter = 0;

        while y.abs_diff(y_prev) > 1 {
            // println!("y_prev = {}", y_prev);
            // println!("y = {}", y);
            // println!("y pow 2 = {}", y.pow(2));
            // println!("c = {}", c);
            // println!("b = {}", b);
            // println!("counter = {}", counter);
            y_prev = y;
            y = (y.pow(2) + c) / (2 * y as i128 + b) as u128;

            counter += 1;
            if counter > 1000 {
                break;
            }
        }

        y
    }

    fn dy(&self, i: usize, j: usize, dx: u128) -> u128 {
        let xp = self.xp();
        xp[j] - self.y(i, j, xp[i] + dx)
    }

    // pub fn sim_dy(&self, i: u128, j: u128, dx: u128) -> u128 {
    //     let gil = Python::acquire_gil();
    //     return self
    //         .call1(gil.python(), "dy", (i, j, dx))
    //         .unwrap()
    //         .extract(gil.python())
    //         .unwrap();
    // }

    pub fn sim_dy(&self, i: u128, j: u128, dx: u128) -> u128 {
        self.dy(i as usize, j as usize, dx)
    }

    fn exchange(&mut self, i: usize, j: usize, dx: u128) -> u128 {
        let xp = self.xp();
        let x = xp[i] + dx;
        let y = self.y(i, j, x);
        let dy = xp[j] - y;
        let fee = dy * self.fee / 10u128.pow(10);

        if dy == 0 {
            return 0;
        }

        self.balances[i] = x * 10u128.pow(18) / self.target_prices[i];
        self.balances[j] = (y + fee) * 10u128.pow(18) / self.target_prices[j];
        dy - fee
    }

    // pub fn sim_exchange(&self, i: u128, j: u128, dx: u128) -> u128 {
    //     let gil = Python::acquire_gil();
    //     return self
    //         .call1(gil.python(), "exchange", (i, j, dx))
    //         .unwrap()
    //         .extract(gil.python())
    //         .unwrap();
    // }

    pub fn sim_exchange(&mut self, i: u128, j: u128, dx: u128) -> u128 {
        self.exchange(i as usize, j as usize, dx)
    }

    pub fn sim_xp(&self) -> Vec<u128> {
        self.xp()
    }

    // pub fn sim_xp(&self) -> Vec<u128> {
    //     let gil = Python::acquire_gil();
    //     return self
    //         .call0(gil.python(), "xp")
    //         .unwrap()
    //         .extract(gil.python())
    //         .unwrap();
    // }

    pub fn sim_y(&self, i: u128, j: u128, x: u128) -> u128 {
        self.y(i as usize, j as usize, x)
    }

    // pub fn sim_y(&self, i: u128, j: u128, x: u128) -> u128 {
    //     let gil = Python::acquire_gil();
    //     return self
    //         .call1(gil.python(), "y", (i, j, x))
    //         .unwrap()
    //         .extract(gil.python())
    //         .unwrap();
    // }

    fn y_d(&self, i: usize, d: u128) -> u128 {
        let mut xx = self.xp();
        xx.remove(i);
        let s: u128 = xx.iter().sum();
        let ann = self.amp_factor * self.n_coins as u128;
        let mut c = d;
        for y in &xx {
            c = c * d / (y * self.n_coins as u128);
        }
        c = c * d / (self.n_coins as u128 * ann);
        let b = s + d / ann;
        let mut y_prev = 0;
        let mut y = d;

        let mut counter = 0;

        while y.abs_diff(y_prev) > 1 {
            y_prev = y;
            y = (y.pow(2) + c) / (2 * y + b - d);

            counter += 1;
            if counter > 1000 {
                break;
            }
        }
        y
    }

    pub fn sim_y_d(&self, i: u128, d: u128) -> u128 {
        self.y_d(i as usize, d)
    }

    // pub fn sim_y_d(&self, i: u128, d: u128) -> u128 {
    //     let gil = Python::acquire_gil();
    //     return self
    //         .call1(gil.python(), "y_D", (i, d))
    //         .unwrap()
    //         .extract(gil.python())
    //         .unwrap();
    // }

    fn remove_liquidity_imbalance(&mut self, amounts: Vec<u128>) -> u128 {
        let _fee = self.fee * self.n_coins as u128 / (4 * (self.n_coins as u128 - 1));

        let old_balances = self.balances.clone();
        let mut new_balances = self.balances.clone();
        let d0 = self.d();
        for i in 0..self.n_coins as usize {
            new_balances[i] -= amounts[i];
        }
        self.balances = new_balances.clone();
        let d1 = self.d();
        self.balances = old_balances.clone();
        let mut fees = vec![0; self.n_coins as usize];
        for i in 0..self.n_coins as usize {
            let ideal_balance = d1 * old_balances[i] / d0;
            let difference = ideal_balance.abs_diff(new_balances[i]);
            fees[i] = _fee * difference / 10u128.pow(10);
            new_balances[i] -= fees[i];
        }
        self.balances = new_balances;
        let d2 = self.d();
        self.balances = old_balances;

        (d0 - d2) * self.pool_tokens / d0
    }

    pub fn sim_remove_liquidity_imbalance(&mut self, amounts: Vec<u128>) -> u128 {
        self.remove_liquidity_imbalance(amounts)
    }

    // pub fn sim_remove_liquidity_imbalance(&self, amounts: Vec<u128>) -> u128 {
    //     let gil = Python::acquire_gil();
    //     return self
    //         .call1(
    //             gil.python(),
    //             "remove_liquidity_imbalance",
    //             PyTuple::new(gil.python(), amounts.to_vec()),
    //         )
    //         .unwrap()
    //         .extract(gil.python())
    //         .unwrap();
    // }

    fn calc_withdraw_one_coin(&self, token_amount: u128, i: usize) -> u128 {
        let xp = self.xp();
        let fee = if self.fee > 0 {
            self.fee - self.fee * xp[i] / xp.iter().sum::<u128>() + 5 * 10u128.pow(5)
        } else {
            0
        };

        let d0 = self.d();
        let d1 = d0 - token_amount * d0 / self.pool_tokens;
        let dy = xp[i] - self.y_d(i, d1);

        dy - dy * fee / 10u128.pow(10)
    }

    pub fn sim_calc_withdraw_one_coin(&self, token_amount: u128, i: u128) -> u128 {
        self.calc_withdraw_one_coin(token_amount, i as usize)
    }

    // fn call0(&self, py: Python, method_name: &str) -> Result<PyObject, PyErr> {
    //     let sim = PyModule::from_code(py, &self.py_src, FILE_NAME, MODULE_NAME).unwrap();
    //     let model = sim
    //         .call_method1("Curve",
    //                (
    //             self.amp_factor,
    //             self.balances.to_vec(),
    //             self.n_coins,
    //             self.target_prices.to_vec(),
    //             self.pool_tokens,
    //             )
    //         )
    //         .unwrap()
    //         .to_object(py);
    //     let py_ret = model.as_ref(py).call_method0(method_name);
    //     self.extract_py_ret(py, py_ret)
    // }

    // fn call1(
    //     &self,
    //     py: Python,
    //     method_name: &str,
    //     args: impl IntoPy<Py<PyTuple>>,
    // ) -> Result<PyObject, PyErr> {
    //     let sim = PyModule::from_code(py, &self.py_src, FILE_NAME, MODULE_NAME).unwrap();
    //     let model = sim
    //         .call_method1("Curve",(
    //             self.amp_factor,
    //             self.balances.to_vec(),
    //             self.n_coins,
    //             self.target_prices.to_vec(),
    //             self.pool_tokens,
    //         ))
    //         .unwrap()
    //         .to_object(py);
    //     let py_ret = model.as_ref(py).call_method1(method_name, args);
    //     self.extract_py_ret(py, py_ret)
    // }

    // fn extract_py_ret(&self, py: Python, ret: PyResult<&PyAny>) -> Result<PyObject, PyErr> {
    //     match ret {
    //         Ok(v) => v.extract(),
    //         Err(e) => {
    //             e.print_and_set_sys_last_vars(py);
    //             panic!("Python execution failed.")
    //         }
    //     }
    // }
}
