---
title: Fleek Network Dependency Graph
---
# Fleek Network Dependency Graph

```mermaid
stateDiagram-v2
  direction LR
  ConfigProviderInterface --> ApplicationInterface
  BlockstoreInterface --> ApplicationInterface
  BlockstoreServerInterface --> ApplicationInterface
  ConfigProviderInterface --> BlockstoreInterface
  ConfigProviderInterface --> BlockstoreServerInterface
  BlockstoreInterface --> BlockstoreServerInterface
  ConfigProviderInterface --> BroadcastInterface
  ApplicationInterface --> BroadcastInterface
  TopologyInterface --> BroadcastInterface
  SignerInterface --> BroadcastInterface
  NotifierInterface --> BroadcastInterface
  ReputationAggregatorInterface --> BroadcastInterface
  ConfigProviderInterface --> TopologyInterface
  SignerInterface --> TopologyInterface
  ApplicationInterface --> TopologyInterface
  ConfigProviderInterface --> ConsensusInterface
  SignerInterface --> ConsensusInterface
  ApplicationInterface --> ConsensusInterface
  BroadcastInterface --> ConsensusInterface
  ConfigProviderInterface --> HandshakeInterface
  ServiceExecutorInterface --> HandshakeInterface
  SignerInterface --> HandshakeInterface
  ApplicationInterface --> NotifierInterface
  ConfigProviderInterface --> OriginProviderInterface
  BlockstoreInterface --> OriginProviderInterface
  ConfigProviderInterface --> DeliveryAcknowledgmentAggregatorInterface
  SignerInterface --> DeliveryAcknowledgmentAggregatorInterface
  ConfigProviderInterface --> ReputationAggregatorInterface
  SignerInterface --> ReputationAggregatorInterface
  NotifierInterface --> ReputationAggregatorInterface
  ApplicationInterface --> ReputationAggregatorInterface
  ConfigProviderInterface --> ResolverInterface
  BroadcastInterface --> ResolverInterface
  SignerInterface --> ResolverInterface
  ConfigProviderInterface --> RpcInterface
  ConsensusInterface --> RpcInterface
  ApplicationInterface --> RpcInterface
  FetcherInterface --> RpcInterface
  ConfigProviderInterface --> ServiceExecutorInterface
  BlockstoreInterface --> ServiceExecutorInterface
  ConfigProviderInterface --> SignerInterface
  ApplicationInterface --> SignerInterface
  ConfigProviderInterface --> FetcherInterface
  BlockstoreInterface --> FetcherInterface
  ResolverInterface --> FetcherInterface
  OriginProviderInterface --> FetcherInterface
```
