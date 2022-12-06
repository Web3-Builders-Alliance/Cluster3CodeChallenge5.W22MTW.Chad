#![cfg(test)]

use cosmwasm_std::{to_binary, Addr, Empty, Uint128, WasmMsg};
use cw20::{BalanceResponse, MinterResponse};
use cw20_base::msg::QueryMsg;
use cw3::Vote;
use cw_multi_test::{App, Contract, ContractWrapper, Executor};
use cw_utils::{Duration, Threshold};

use crate::contract::{execute, instantiate, query};
use crate::msg::{ExecuteMsg, InstantiateMsg, Voter};

use counter::{self, helpers::CounterContract};

fn mock_app() -> App {
    App::default()
}

pub fn contract_cw3_fixed_multisig() -> Box<dyn Contract<Empty>> {
    let contract = ContractWrapper::new(execute, instantiate, query);
    Box::new(contract)
}

pub fn contract_cw20() -> Box<dyn Contract<Empty>> {
    let contract = ContractWrapper::new(
        cw20_base::contract::execute,
        cw20_base::contract::instantiate,
        cw20_base::contract::query,
    );
    Box::new(contract)
}

pub fn contract_counter() -> Box<dyn Contract<Empty>> {
    let contract = ContractWrapper::new(
        counter::contract::execute,
        counter::contract::instantiate,
        counter::contract::query,
    );
    Box::new(contract)
}

fn store_code() -> (App, u64, u64) {
    let mut app = mock_app();
    let cw3_multisig = app.store_code(contract_cw3_fixed_multisig());
    let counter_id = app.store_code(contract_counter());
    (app, cw3_multisig, counter_id)
}

#[test]
// cw3 multisig account can control cw20 admin actions
fn cw3_controls_cw20() {
    let mut router = mock_app();

    // setup cw3 multisig with 3 accounts
    let cw3_id = router.store_code(contract_cw3_fixed_multisig());

    let addr1 = Addr::unchecked("addr1");
    let addr2 = Addr::unchecked("addr2");
    let addr3 = Addr::unchecked("addr3");
    let cw3_instantiate_msg = InstantiateMsg {
        voters: vec![
            Voter {
                addr: addr1.to_string(),
                weight: 1,
            },
            Voter {
                addr: addr2.to_string(),
                weight: 1,
            },
            Voter {
                addr: addr3.to_string(),
                weight: 1,
            },
        ],
        threshold: Threshold::AbsoluteCount { weight: 2 },
        max_voting_period: Duration::Height(3),
    };

    let multisig_addr = router
        .instantiate_contract(
            cw3_id,
            addr1.clone(),
            &cw3_instantiate_msg,
            &[],
            "Consortium",
            None,
        )
        .unwrap();

    // setup cw20 as cw3 multisig admin
    let cw20_id = router.store_code(contract_cw20());

    let cw20_instantiate_msg = cw20_base::msg::InstantiateMsg {
        name: "Consortium Token".parse().unwrap(),
        symbol: "CST".parse().unwrap(),
        decimals: 6,
        initial_balances: vec![],
        mint: Some(MinterResponse {
            minter: multisig_addr.to_string(),
            cap: None,
        }),
        marketing: None,
    };
    let cw20_addr = router
        .instantiate_contract(
            cw20_id,
            multisig_addr.clone(),
            &cw20_instantiate_msg,
            &[],
            "Consortium",
            None,
        )
        .unwrap();

    // mint some cw20 tokens according to proposal result
    let mint_recipient = Addr::unchecked("recipient");
    let mint_amount = Uint128::new(1000);
    let cw20_mint_msg = cw20_base::msg::ExecuteMsg::Mint {
        recipient: mint_recipient.to_string(),
        amount: mint_amount,
    };

    let execute_mint_msg = WasmMsg::Execute {
        contract_addr: cw20_addr.to_string(),
        msg: to_binary(&cw20_mint_msg).unwrap(),
        funds: vec![],
    };
    let propose_msg = ExecuteMsg::Propose {
        title: "Mint tokens".to_string(),
        description: "Need to mint tokens".to_string(),
        msgs: vec![execute_mint_msg.into()],
        latest: None,
    };
    // propose mint
    router
        .execute_contract(addr1.clone(), multisig_addr.clone(), &propose_msg, &[])
        .unwrap();

    // second votes
    let vote2_msg = ExecuteMsg::Vote {
        proposal_id: 1,
        vote: Vote::Yes,
    };
    router
        .execute_contract(addr2, multisig_addr.clone(), &vote2_msg, &[])
        .unwrap();

    // only 1 vote and msg mint fails
    let execute_proposal_msg = ExecuteMsg::Execute { proposal_id: 1 };
    // execute mint
    router
        .execute_contract(addr1, multisig_addr, &execute_proposal_msg, &[])
        .unwrap();

    // check the mint is successful
    let cw20_balance_query = QueryMsg::Balance {
        address: mint_recipient.to_string(),
    };
    let balance: BalanceResponse = router
        .wrap()
        .query_wasm_smart(&cw20_addr, &cw20_balance_query)
        .unwrap();

    // compare minted amount
    assert_eq!(balance.balance, mint_amount);
}

#[test]
fn cw3_3_of_5_multisig() {
    let mut router = mock_app();

    // setup cw3 multisig with 5 accounts
    let cw3_id = router.store_code(contract_cw3_fixed_multisig());

    let addr1 = Addr::unchecked("addr1");
    let addr2 = Addr::unchecked("addr2");
    let addr3 = Addr::unchecked("addr3");
    let addr4 = Addr::unchecked("addr4");
    let addr5 = Addr::unchecked("addr5");

    let cw3_instantiate_msg = InstantiateMsg {
        voters: vec![
            Voter {
                addr: addr1.to_string(),
                weight: 1,
            },
            Voter {
                addr: addr2.to_string(),
                weight: 1,
            },
            Voter {
                addr: addr3.to_string(),
                weight: 1,
            },
            Voter {
                addr: addr4.to_string(),
                weight: 1,
            },
            Voter {
                addr: addr5.to_string(),
                weight: 1,
            },
        ],
        threshold: Threshold::AbsoluteCount { weight: 3 },
        max_voting_period: Duration::Height(3),
    };

    let multisig_addr = router
        .instantiate_contract(
            cw3_id,
            addr1.clone(),
            &cw3_instantiate_msg,
            &[],
            "3 of 5 multisig",
            None,
        )
        .unwrap();

    // setup cw20 as cw3 multisig admin
    let counter_id = router.store_code(contract_counter());

    let counter_instantiate_msg = counter::msg::InstantiateMsg {
        count: 0,
        owner: multisig_addr.to_string().clone(),
    };
    let counter_addr = router
        .instantiate_contract(
            counter_id,
            multisig_addr.clone(),
            &counter_instantiate_msg,
            &[],
            "3 of 5 counter",
            None,
        )
        .unwrap();

    // increment counter according to proposal result
    let counter_increment_msg = counter::msg::ExecuteMsg::Increment {};

    let execute_increment_msg = WasmMsg::Execute {
        contract_addr: counter_addr.to_string(),
        msg: to_binary(&counter_increment_msg).unwrap(),
        funds: vec![],
    };
    let propose_msg = ExecuteMsg::Propose {
        title: "Increment".to_string(),
        description: "Let's increment the counter!".to_string(),
        msgs: vec![execute_increment_msg.into()],
        latest: None,
    };
    // propose increment
    router
        .execute_contract(addr1.clone(), multisig_addr.clone(), &propose_msg, &[])
        .unwrap();

    // second vote
    let vote2_msg = ExecuteMsg::Vote {
        proposal_id: 1,
        vote: Vote::Yes,
    };
    let res = router.execute_contract(addr2, multisig_addr.clone(), &vote2_msg, &[]);
    assert!(res.is_ok());

    // only 2 votes and msg increment fails
    let execute_proposal_msg = ExecuteMsg::Execute { proposal_id: 1 };
    // execute increment
    let res = router.execute_contract(
        addr1.clone(),
        multisig_addr.clone(),
        &execute_proposal_msg,
        &[],
    );
    assert!(res.is_err());

    // check if increment is successful
    let counter_value_query = counter::msg::QueryMsg::GetCount {};
    let count: counter::msg::GetCountResponse = router
        .wrap()
        .query_wasm_smart(&counter_addr, &counter_value_query)
        .unwrap();

    // compare counter
    assert_eq!(count.count, 0);

    let vote3_msg = ExecuteMsg::Vote {
        proposal_id: 1,
        vote: Vote::Yes,
    };
    let res = router.execute_contract(addr3, multisig_addr.clone(), &vote3_msg, &[]);
    assert!(res.is_ok());

    // now 3 votes and msg increment passes!
    let execute_proposal_msg = ExecuteMsg::Execute { proposal_id: 1 };
    // execute increment
    let res = router.execute_contract(
        addr1.clone(),
        multisig_addr.clone(),
        &execute_proposal_msg,
        &[],
    );
    assert!(res.is_ok());

    // check if increment is successful
    let counter_value_query = counter::msg::QueryMsg::GetCount {};
    let count: counter::msg::GetCountResponse = router
        .wrap()
        .query_wasm_smart(&counter_addr, &counter_value_query)
        .unwrap();

    // compare counter
    assert_eq!(count.count, 1);
}

//CW3 Controls Counter
// #[test]
// fn cw3_controls_counter() {
//     //CODING CHALLENEGE
//     unimplemented!()
// }
