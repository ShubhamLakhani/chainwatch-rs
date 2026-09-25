// SPDX-License-Identifier: MIT
pragma solidity ^0.8.20;

import {DemoToken} from "../src/DemoToken.sol";

/// @notice Minimal Foundry test without forge-std so this subproject stays self-contained.
contract DemoTokenTest {
    function test_constructor_mints_supply_to_deployer() public {
        DemoToken token = new DemoToken(1_000 ether);
        require(token.totalSupply() == 1_000 ether, "supply");
        require(token.balanceOf(address(this)) == 1_000 ether, "balance");
        require(token.owner() == address(this), "owner");
    }

    function test_transfer_moves_balance() public {
        DemoToken token = new DemoToken(100 ether);
        address recipient = address(0xBEEF);
        bool ok = token.transfer(recipient, 10 ether);
        require(ok, "transfer");
        require(token.balanceOf(address(this)) == 90 ether, "sender");
        require(token.balanceOf(recipient) == 10 ether, "recipient");
    }
}
