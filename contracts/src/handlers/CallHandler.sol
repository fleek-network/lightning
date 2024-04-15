// SPDX-License-Identifier: MIT
pragma solidity ^0.8.13;

import {AbstractHandler} from "src/handlers/AbstractHandler.sol";
import {Clones} from "@openzeppelin/contracts/proxy/Clones.sol";
import {CallFailed, UnauthorizedSender} from "src/Errors.sol";

struct CallHandlerIncomingMessage {
    address sender;
    address target;
    bytes data;
}

/// The handler in charge of passing arbitray messges between fleek and ethereum
contract CallHandler is AbstractHandler {
    /// @notice map the senders from the other side of the bridge to an indivual address
    /// @dev needed to avoid auth vulns
    mapping(address => Puppet) public puppet;

    /// @notice The puppet implementation deployed by this
    address public immutable puppetImpl;

    constructor(address _messageCenter) AbstractHandler(_messageCenter) {
        puppetImpl = address(new Puppet());
    }

    /// @notice No op, all data is valid
    function handleOutgoingMessage(bytes calldata) external view override {
        _revertIfNotMessageCenter();
    }

    /// @dev Handle messages from the message center
    /// @dev implementors should take care to validate the sender of the message is the message center
    function handleIncomingMessage(bytes calldata data) external override {
        _revertIfNotMessageCenter();
        CallHandlerIncomingMessage memory message = abi.decode(data, (CallHandlerIncomingMessage));

        if (address(puppet[message.sender]) == address(0)) {
            puppet[message.sender] = Puppet(Clones.clone(puppetImpl));
        }

        puppet[message.sender].initCall(message.target, message.data);
    }
}

contract Puppet {
    address public immutable owner;

    constructor() {
        owner = msg.sender;
    }

    function initCall(address target, bytes calldata data) external {
        if (msg.sender != owner) revert UnauthorizedSender();

        (bool success,) = target.call(data);
        if (!success) revert CallFailed();
    }
}
