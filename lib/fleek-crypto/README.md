# Fleek Crypto

A light wrapper around the [fastcrypto](https://github.com/MystenLabs/fastcrypto) crate, providing simple 
PrivateKey and PublicKey traits, with basic PEM encoding and decoding functionality added.

## Key Types

In fleek network, there are a few different key types:

- Account Owner Keys: Secp256k1, which used for balances. They use recoverable signatures, and can be converted into an ethereum address.
- Client Keys: To be determined, used for clients to sign proof of deliveries.
- Node Keys: BLS 12-381, which are used by consensus. The public key variant is 96 bytes.
- Node Networking Keys: Ed25519, which are used for the DHT, gossip/pubsub, etc.

