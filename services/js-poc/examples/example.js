import { Web3 } from 'https://esm.sh/v135/web3@4.9.0/es2022/web3.bundle.mjs#integrity=sha256-BcoVq5eJ5COshAwMRhgNB4AaM3KnY9mNxM6NDaa/7qc='

export const main = () => {
  const web3 = new Web3(
    "wss://eth-mainnet.g.alchemy.com/v2/kkd9LJZTw2aZZ12DbeZDUTymIoRifxqW"
  );

  // ABI of your contract including the new event
  const abi = [
    {
      constant: true,
      inputs: [{ name: "proxy", type: "address" }],
      name: "readDataFeed",
      outputs: [
        { name: "value", type: "int224" },
        { name: "timestamp", type: "uint32" },
      ],
      payable: false,
      stateMutability: "view",
      type: "function",
    },
    {
      anonymous: false,
      inputs: [
        { indexed: true, name: "proxy", type: "address" },
        { indexed: false, name: "value", type: "int224" },
        { indexed: false, name: "timestamp", type: "uint32" },
      ],
      name: "DataFeedRead",
      type: "event",
    },
  ];

  // Address of your deployed contract
  const contractAddress = "0xAB8e46012957020B9ECEFFA73B453D396A8d4738";

  // Create a contract instance
  const contract = new web3.eth.Contract(abi, contractAddress);

  return new Promise((resolve, reject) => {
    let messages = [];

    // Listen for DataFeedRead events
    contract.events.DataFeedRead(
      {
        filter: {}, // Add any filters if needed
        fromBlock: "latest",
      },
      (error, event) => {
        if (error) {
          console.error("Error on event", error);
          reject(error);
        } else {
          console.log("Event data:", event.returnValues);
          messages.push(event.returnValues);
          resolve(messages); // Resolve the promise with messages
        }
      }
    );
  });
};
