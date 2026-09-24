// SPDX-License-Identifier: MIT
pragma solidity ^0.8.20;

import {DemoToken} from "../src/DemoToken.sol";

/// @notice Deployment helper for local Anvil. Prefer `forge create` from
///         `scripts/dev/seed.sh` so this subproject does not require forge-std.
///
/// Example (from the `contracts/` directory):
/// `forge create src/DemoToken.sol:DemoToken --rpc-url $RPC_URL --private-key $PRIVATE_KEY --broadcast --constructor-args 1000000000000000000000000`
contract DeployDemoToken {
    function deploy(uint256 initialSupply) external returns (DemoToken token) {
        token = new DemoToken(initialSupply);
    }
}
