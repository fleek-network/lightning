import {
  ClientPublicKey,
  ClientSignature,
  HandshakeRequest,
  RawAccessToken,
  Request,
  Response,
  ServiceId,
} from "../schema.ts";

/**
 * A webRTC client.
 */
export class FleekRTC {
  readonly SDPAddr: string;
  peer: RTCPeerConnection;
  firstDataChannel: RTCDataChannel;
  initialized: boolean;
  id: number;

  /**
   * @param {string} ip - A URL to a node's SDP endpoint.
   * @param {RTCIceServer[]} iceServers - Optional list of ICE servers to use.
   */
  constructor(
    readonly ip: string,
    iceServers?: RTCIceServer[],
  ) {
    this.SDPAddr = "http://" + ip + ":4220/sdp";
    this.initialized = false;
    this.id = 0;

    this.peer = new RTCPeerConnection({
      iceServers: iceServers || [
        {
          urls: "stun:stun.l.google.com:19302",
        },
      ],
    });

    this.peer.onnegotiationneeded = async () => {
      console.log("creating sdp offer");
      const offer = await this.peer.createOffer();
      await this.peer.setLocalDescription(offer);
    };

    this.peer.oniceconnectionstatechange = () => {
      console.log("ice status: ", this.peer.iceConnectionState);
    };

    this.peer.onicegatheringstatechange = (e) => {
      console.log("gather: ", e);
    };

    this.firstDataChannel = this.peer.createDataChannel(`fleek-${this.id}`, {
      ordered: true,
    });
  }

  /** Connect to the node, negotiating over the SDP URL given to the constructor */
  async connect(): Promise<FleekRTCStream> {
    let channel: RTCDataChannel;
    if (!this.initialized) {
      // negotiate a connection over SDP and use the initializing data channel
      console.log(this.SDPAddr);
      const res = await fetch(this.SDPAddr, {
        method: "POST",
        headers: {
          "Content-Type": "application/json",
        },
        body: JSON.stringify(this.peer.localDescription),
      });

      const sd = await res.json();
      if (sd === "") {
        throw "invalid sdp response";
      }

      this.peer.setRemoteDescription(new RTCSessionDescription(sd));
      this.initialized = true;
      channel = this.firstDataChannel;
    } else {
      // create an additional data channel
      this.id += 1;
      channel = this.peer.createDataChannel(`fleek-${this.id}`, {
        ordered: true,
      });
    }

    // promise-ify the channel open callback
    await new Promise((resolve, reject) => {
      channel.onopen = () => {
        resolve(null);
      };
      channel.onerror = (e) => {
        reject(e);
      };
    });

    return new FleekRTCStream(channel);
  }

  close() {
    this.peer.close();
  }
}

export class FleekRTCStream {
  channel: RTCDataChannel;
  queue: (Response.Frame | undefined)[];
  currentPromise: DeferredRecv | undefined;
  initialized: boolean;

  constructor(channel: RTCDataChannel) {
    this.channel = channel;
    this.queue = [];
    this.initialized = false;
  }

  handshakePrimary(serviceId: ServiceId | number): Promise<void> {
    if (this.initialized) {
      throw "stream already initialized";
    }

    this.channel.send(HandshakeRequest.encode({
      tag: HandshakeRequest.Tag.Handshake,
      service: serviceId as ServiceId,
      // TODO: cryptography
      pk: new Uint8Array(96) as ClientPublicKey,
      pop: new Uint8Array(48) as ClientSignature,
    }));

    // TODO: read and verify HandshakeResponse once the server implementation sends it

    this.setup();
    return Promise.resolve();
  }

  handshakeSecondary(accessToken: Uint8Array): Promise<void> {
    if (this.initialized) {
      throw "stream already initialized";
    }

    this.channel.send(HandshakeRequest.encode({
      tag: HandshakeRequest.Tag.JoinRequest,
      accessToken: accessToken as RawAccessToken,
    }));

    // TODO: read and verify HandshakeResponse once the server implementation sends it

    this.setup();
    return Promise.resolve();
  }

  /** Send a request frame to the node
   * @param {Request.Frame} frame - The frame to send
   */
  send(frame: Request.Frame): Promise<void> {
    this.channel.send(Request.encode(frame));
    // interface compatibility
    return Promise.resolve();
  }

  /** Receive a frame from the node
   * @returns Promise<Response.Frame | undefined>
   */
  recv(): Promise<Response.Frame | undefined> {
    if (this.queue.length != 0) {
      // immediately resolve with any queued frames
      return Promise.resolve(this.queue.shift());
    }

    // create a deferred promise to wait for the next message
    this.currentPromise = new DeferredRecv();
    return this.currentPromise.promise;
  }

  private setup() {
    this.channel.onmessage = (e: MessageEvent) => {
      const frame = Response.decode(e.data);
      // todo: should we buffer all chunked payloads here?
      if (this.currentPromise) {
        this.currentPromise.resolve(frame);
        this.currentPromise = undefined;
      } else {
        this.queue.push(frame);
      }
    };

    this.initialized = true;
  }
}

class DeferredRecv {
  promise: Promise<Response.Frame | undefined>;

  // @ts-ignore 2564
  resolve: (
    res: Response.Frame | PromiseLike<Response.Frame | undefined> | undefined,
  ) => void;
  // @ts-ignore 2564
  // deno-lint-ignore no-explicit-any
  reject: (reason?: any) => void;

  constructor() {
    this.promise = new Promise((resolve, reject) => {
      this.reject = reject;
      this.resolve = resolve;
    });
  }
}
