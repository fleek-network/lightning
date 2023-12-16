import {
  ClientPublicKey,
  ClientSignature,
  HandshakeRequest,
  RawAccessToken,
  Request,
  Response,
  ServiceId,
} from "../schema.ts";

/** Fleek WebTransport client */
export class FleekTransport {
  readonly transportAddr: string;
  readonly hashAddr: string;
  hash: ArrayBuffer | undefined;
  transport: WebTransport | undefined;

  /**
   * @param {string} ip - A node's IP address, which should be bound to standard port assignments
   * @param {Uint8Array} hash - (Optional) Pre-fetched certificate hash
   */
  constructor(readonly ip: string, hash?: ArrayBuffer) {
    this.transportAddr = "https://" + ip + ":4321";
    this.hashAddr = "http://" + ip + ":4220/certificate-hash";
    if (hash) {
      this.hash = hash;
    }
  }

  /** Create a new connection stream.
   * The certificate hash is fetched if it's not known yet.
   * Initializes a parent webtransport connection if one hasn't been created.
   */
  async connect(): Promise<FleekTransportStream> {
    if (!this.hash) {
      // Fetch the server's certificate hash
      this.hash = await (await fetch(this.hashAddr)).arrayBuffer();
    }

    if (!this.transport) {
      this.transport = new WebTransport(this.transportAddr, {
        serverCertificateHashes: [{ algorithm: "sha-256", value: this.hash }],
      });
      await this.transport.ready;
    }

    // Open a bi-directional stream.
    const stream = await this.transport.createBidirectionalStream();
    return new FleekTransportStream(stream);
  }

  /** Close all streams on the transport */
  close(info?: WebTransportCloseInfo | undefined) {
    this.transport?.close(info);
  }
}

/** A unique connection stream to a fleek node
 * Consumers *MUST* call `handshakePrimary()` or `handshakeSecondary()` before using any other method.
 */
export class FleekTransportStream {
  stream: WebTransportBidirectionalStream;
  writer: WritableStreamDefaultWriter;
  reader: ReadableStreamDefaultReader;
  buffer: Uint8Array;

  constructor(stream: WebTransportBidirectionalStream) {
    this.stream = stream;
    this.writer = this.stream.writable.getWriter();
    this.reader = this.stream.readable.getReader();
    this.buffer = new Uint8Array();
  }

  /** Handshake with the node as the primary connection for the session.
   * @param {ServiceId} serviceId - Identifier for the service to connect to.
   */
  async handshakePrimary(serviceId: ServiceId | number) {
    await this.sendInner(
      HandshakeRequest.encode({
        tag: HandshakeRequest.Tag.Handshake,
        service: serviceId as ServiceId,
        // TODO: cryptography
        pk: new Uint8Array(96) as ClientPublicKey,
        pop: new Uint8Array(48) as ClientSignature,
      }),
    );
    // TODO: read and verify HandshakeResponse once the server implementation sends it
  }

  /** Handshake with the node as the secondary connection for the session.
   * @param {Uint8Array} accessToken - The token granted to the primary connection for the session.
   */
  async handshakeSecondary(accessToken: Uint8Array) {
    await this.sendInner(HandshakeRequest.encode({
      tag: HandshakeRequest.Tag.JoinRequest,
      accessToken: accessToken as RawAccessToken,
    }));
    // TODO: read and verify HandshakeResponse once the server implementation sends it
  }

  /** Send a request frame to the node
   * @param {Request.Frame} frame - The frame to send
   */
  send(frame: Request.Frame): Promise<void> {
    return this.sendInner(Request.encode(frame));
  }

  /** Receive a frame from the node
   * @returns {Promise<Response.Frame | undefined>}
   */
  async recv(): Promise<Response.Frame | undefined> {
    const payload = await this.recvInner();
    if (!payload) return;
    return Response.decode(payload);
  }

  private async sendInner(buffer: ArrayBuffer) {
    const length = buffer.byteLength;
    const delim = new DataView(new ArrayBuffer(4));
    delim.setUint32(0, length, false);

    await this.writer.write(delim);
    await this.writer.write(buffer);
  }

  private async readUntil(len: number) {
    while (this.buffer.byteLength < len) {
      const { value, done } = await this.reader.read();
      const len: number = this.buffer.length + value.length;
      const tmp = new Uint8Array(len);
      tmp.set(this.buffer, 0);
      tmp.set(value, this.buffer.length);
      this.buffer = tmp;

      if (done && this.buffer.length < len) {
        throw "stream terminated early";
      }
    }
  }

  private async recvInner(): Promise<ArrayBuffer | undefined> {
    // Read until we have enough bytes to parse the length delimiter
    await this.readUntil(4);
    const view = new DataView(this.buffer.buffer.slice(0, 4));
    const lengthPrefix = view.getUint32(0, false);
    this.buffer = this.buffer.slice(4);

    // Read until we have enough bytes to parse the frame
    await this.readUntil(lengthPrefix);
    const payload = this.buffer.buffer.slice(0, lengthPrefix);
    this.buffer = this.buffer.slice(lengthPrefix);

    return payload;
  }
}
