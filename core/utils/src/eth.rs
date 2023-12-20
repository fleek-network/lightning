use ethers::prelude::abigen;
abigen!(
    FleekContract,
    r"[
        function withdraw(uint256 amount, string token, address recipient)
        function stake(uint256 amount, bytes32 nodePublicKey, bytes consensusKey, string domain, bytes32 workerPublicKey, string workerDomain)
        function deposit(string token, uint256 amount)
        function unstake(uint256 amount, bytes32 node_public_key)
        function withdrawUnstaked(bytes32 node_public_key, address recipient)
    ]"
);
