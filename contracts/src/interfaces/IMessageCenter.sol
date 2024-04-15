// SPDX-License-Identifier: MIT
pragma solidity ^0.8.13;

interface IMessageCenter {
    event FlkMessage(address indexed sender, uint256 typeOf, bytes data);
}
