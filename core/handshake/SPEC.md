# Service Handshake Protocol

Nodes allow for a certain number of lanes for a client to run services within.
The handshake protocol provides a way for a client to request a lane, or unlock a disconnected lane,
and initiate a service.

```mermaid
sequenceDiagram
    actor Client
    participant Node
    Client->>Node: Handshake request
    Node->>Client: Handshake response
    opt Should unlock?
      Client->>Node: Acknowledgement Signature
    end
    loop service loop
			Client->>Node: Service Request
			Client-->Node: Service Subprotocol
	      Node->>Client: Subprotocol completes
    end
```

## Codec

### Frame Tags

Each frame is prefixed by a 1 byte tag. The signal flag will always be set to 0.

```
0 [ 0 0 0 0 0 0 0 ] - tag bitmap
 \_ signal flag
```

### Termination Signals

When the connection can be gracefully terminated, a client or server can provide a 1 byte signal with a reason.
The signal flag will always be set to 1.

```
1 [ 0 0 0 0 0 0 0 ] - reason bits
 \_ signal flag

Termination Reasons:
0x80 & 0: Codec Violation
0x80 & 1: Out of Lanes
0x80 & 2: ...
0x80 & 127: Unknown
```

### Handshake Request Frame

```
TAG = 0x01 << 0

[ TAG . b"UFDP" . version . supported compression bitmap . pubkey . lane ]

Supported Compression Bits:
SNAPPY = 0x01 << 0
GZIP = 0x01 << 1
LZ4 = 0x01 << 2

If lane is 0xFF, the node should select an open lane for the client. Otherwise,
the lane provided should be unlocked.
```

### Handshake Response Frame

```
TAG = 0x01 << 1

[ TAG . lane . node pubkey ]
```

### Handshake Response Frame (Unlock Lane)

```
TAG = 0x01 << 2

[ TAG . lane . node pubkey . lane . last service id . last params ]
```
