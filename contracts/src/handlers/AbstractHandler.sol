// SPDX-License-Identifier: MIT
pragma solidity ^0.8.13;

import {UnauthorizedSender} from "src/Errors.sol";

abstract contract AbstractHandler {
    address public immutable messageCenter;

    constructor(address _messageCenter) {
        messageCenter = _messageCenter;
    }

    modifier onlyMessageCenter() {
        _revertIfNotMessageCenter();
        _;
    }

    /// @dev Handle messages from the message center
    /// @dev implementors should take care to validate the sender of the message is the message center
    function handleOutgoingMessage(bytes calldata data) external virtual;

    /// @dev Handle messages from the message center
    /// @dev implementors should take care to validate the sender of the message is the message center
    function handleIncomingMessage(bytes calldata) external virtual {
        _revertIfNotMessageCenter();
    }

    /// @dev Reverts if the sender is not the message center
    /// @dev this should really only be required for the handleIncomingMessage function
    /// @dev but you should probably use it for outgoing as well to prevent LOF since message event will be emitted
    function _revertIfNotMessageCenter() internal view {
        if (msg.sender != messageCenter) revert UnauthorizedSender();
    }
}
