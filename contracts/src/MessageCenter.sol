// SPDX-License-Identifier: MIT
pragma solidity ^0.8.13;

import {IMessageCenter} from "src/interfaces/IMessageCenter.sol";
import {AbstractHandler} from "src/handlers/AbstractHandler.sol";
import {UnauthorizedSender, NoHandlerForMessageType} from "src/Errors.sol";
import {Ownable} from "@openzeppelin/contracts/access/Ownable.sol";

struct OutgoingMessage {
    uint256 messageType;
    bytes data;
}

/// @notice the main entrypoint for messages between FLK and ETH
contract MessageCenter is Ownable, IMessageCenter {
    /// @notice The incoming message
    address public incomingMessage;

    /// @notice The mapping of message types to their respective handlers
    mapping(uint256 => address) public messageHandler;

    constructor(address _incomingMessage, address _owner) Ownable(_owner) {
        incomingMessage = _incomingMessage;
    }

    /// @notice Handle outgoing messages
    function handleOutgoingMessage(OutgoingMessage memory message) external {
        address handler = messageHandler[message.messageType];

        if (handler == address(0)) revert NoHandlerForMessageType();

        AbstractHandler(handler).handleOutgoingMessage(message.data);

        emit FlkMessage(msg.sender, message.messageType, message.data);
    }

    /// @notice Handle incoming messages
    /// @dev Only the incoming message can call this
    function handleIncomingMessage(uint256 messageType, bytes calldata data) external {
        if (msg.sender != incomingMessage) revert UnauthorizedSender();

        AbstractHandler(messageHandler[messageType]).handleIncomingMessage(data);
    }

    /// @notice Sets the handler for a given message type
    function setMessageHandler(uint256 messageType, address handler) external onlyOwner {
        messageHandler[messageType] = handler;
    }

    /// @notice Unsets the handler for a given message type
    function unsetMessageHandler(uint256 messageType) external onlyOwner {
        messageHandler[messageType] = address(0);
    }

    /// @notice Sets the incoming message
    function setIncomingMessage(address _incomingMessage) external onlyOwner {
        incomingMessage = _incomingMessage;
    }
}
