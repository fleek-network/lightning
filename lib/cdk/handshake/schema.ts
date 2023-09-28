// deno-lint-ignore-file no-namespace
/// <reference types="../typeutils.d.ts" />

export type Digest = Opaque<"Digest", Uint8Array>;

export type ConnectionId = Opaque<"ConnectionId", number>;

export type ServiceId = Opaque<"ServiceId", number>;

export type ClientPublicKey = Opaque<"ClientPublicKey", Uint8Array>;

export type ClientSignature = Opaque<"ClientSignature", Uint8Array>;

export type NodeSignature = Opaque<"NodeSignature", Uint8Array>;

export type NodePublicKey = Opaque<"NodePublicKey", Uint8Array>;

export type RawAccessToken = Opaque<"AccessToken", Uint8Array>;

export namespace Challenge {
  // FLEEK.
  export const PREFIX = [102, 108, 101, 101, 107];

  export interface Frame {
    challenge: Digest;
  }

  export function encode(frame: Frame): ArrayBuffer {
    const view = new Uint8Array(37);
    view.set(PREFIX);
    view.set(frame.challenge, 5);
    return view.buffer;
  }

  export function decode(payload: ArrayBuffer): Frame | undefined {
    if (payload.byteLength != 37) {
      return undefined;
    }

    const u8 = new Uint8Array(payload);

    for (let i = 0; i < PREFIX.length; ++i) {
      if (u8[i] != PREFIX[i]) {
        return undefined;
      }
    }

    return {
      challenge: new Uint8Array(payload, 5, 32) as Digest,
    };
  }
}

export namespace HandshakeRequest {
  export type Frame =
    | Handshake
    | JoinRequest;

  export enum Tag {
    Handshake,
    JoinRequest,
  }

  export interface Handshake {
    readonly tag: Tag.Handshake;
    retry?: ConnectionId;
    service: ServiceId;
    pk: ClientPublicKey;
    pop: ClientSignature;
  }

  export interface JoinRequest {
    readonly tag: Tag.JoinRequest;
    accessToken: RawAccessToken;
  }

  export function encode(frame: Frame): ArrayBuffer {
    if (frame.tag === Tag.Handshake) {
      let writer: Writer;
      if (frame.retry === undefined) {
        writer = new Writer(149);
        writer.putU8(0x00);
      } else {
        writer = new Writer(157);
        writer.putU8(0x01);
        writer.putU64(frame.retry);
      }

      writer.putU32(frame.service);
      writer.put(frame.pk);
      writer.put(frame.pop);
      return writer.getBuffer();
    }

    if (frame.tag === Tag.JoinRequest) {
      const u8 = new Uint8Array(49);
      u8[0] = 0x02;
      u8.set(frame.accessToken, 0);
      return u8.buffer;
    }

    throw new Error("Unsupported");
  }

  export function decode(payload: ArrayBuffer): Frame | undefined {
    const reader = new Reader(payload);
    const tag = reader.getU8();

    if (tag === 0x00) {
      if (payload.byteLength !== 149) {
        return;
      }

      return {
        tag: Tag.Handshake,
        service: reader.getU32() as ServiceId,
        pk: reader.get(96) as ClientPublicKey,
        pop: reader.get(48) as ClientSignature,
      };
    }

    if (tag === 0x01) {
      if (payload.byteLength !== 157) {
        return;
      }

      return {
        tag: Tag.Handshake,
        retry: reader.getU64() as ConnectionId,
        service: reader.getU32() as ServiceId,
        pk: reader.get(96) as ClientPublicKey,
        pop: reader.get(48) as ClientSignature,
      };
    }

    if (tag === 0x02) {
      if (payload.byteLength !== 49) {
        return;
      }

      return {
        tag: Tag.JoinRequest,
        accessToken: reader.get(49) as RawAccessToken,
      };
    }
  }
}

export namespace HandshakeResponse {
  export interface Frame {
    pk: NodePublicKey;
    pop: NodeSignature;
  }

  export function encode(frame: Frame): ArrayBuffer {
    const writer = new Writer(96);
    writer.put(frame.pk);
    writer.put(frame.pop);
    return writer.getBuffer();
  }

  export function decode(payload: ArrayBuffer): Frame | undefined {
    if (payload.byteLength != 96) {
      return;
    }

    const reader = new Reader(payload);
    const pk = reader.get(32) as NodePublicKey;
    const pop = reader.get(64) as NodeSignature;
    return { pk, pop };
  }
}

export namespace Request {
  export type Frame =
    | ServicePayload
    | AccessToken
    | ExtendAccessToken;

  export enum Tag {
    ServicePayload,
    AccessToken,
    ExtendAccessToken,
  }

  export interface ServicePayload {
    readonly tag: Tag.ServicePayload;
    bytes: Uint8Array;
  }

  export interface AccessToken {
    readonly tag: Tag.AccessToken;
    ttl: number;
  }

  export interface ExtendAccessToken {
    readonly tag: Tag.ExtendAccessToken;
    ttl: number;
  }

  export function encode(frame: Frame): ArrayBuffer {
    if (frame.tag == Tag.ServicePayload) {
      const u8 = new Uint8Array(frame.bytes.byteLength + 1);
      u8[0] = 0x00;
      u8.set(frame.bytes, 1);
      return u8.buffer;
    }

    if (frame.tag == Tag.AccessToken) {
      const writer = new Writer(9);
      writer.putU8(0x01);
      writer.putU64(frame.ttl);
      return writer.getBuffer();
    }

    if (frame.tag == Tag.ExtendAccessToken) {
      const writer = new Writer(9);
      writer.putU8(0x02);
      writer.putU64(frame.ttl);
      return writer.getBuffer();
    }

    throw new Error("Not supported.");
  }

  export function decode(payload: ArrayBuffer): Frame | undefined {
    const reader = new Reader(payload);
    const tag = reader.getU8();

    if (tag == 0) {
      return {
        tag: Tag.ServicePayload,
        bytes: new Uint8Array(payload).slice(1),
      };
    }

    if (tag == 1) {
      if (payload.byteLength != 9) {
        return;
      }

      return {
        tag: Tag.AccessToken,
        ttl: reader.getU64(),
      };
    }

    if (tag == 2) {
      if (payload.byteLength != 9) {
        return;
      }

      return {
        tag: Tag.ExtendAccessToken,
        ttl: reader.getU64(),
      };
    }
  }
}

export namespace Response {
  export type Frame =
    | ServicePayload
    | ChunkedServicePayload // webrtc only
    | AccessToken
    | Termination;

  export enum Tag {
    ServicePayload,
    ChunkedServicePayload,
    AccessToken,
    Termination,
  }

  export interface ServicePayload {
    readonly tag: Tag.ServicePayload;
    bytes: Uint8Array;
  }

  export interface ChunkedServicePayload {
    readonly tag: Tag.ChunkedServicePayload;
    bytes: Uint8Array;
  }

  export interface AccessToken {
    readonly tag: Tag.AccessToken;
    ttl: number;
    accessToken: RawAccessToken;
  }

  export interface Termination {
    readonly tag: Tag.Termination;
    reason: TerminationReason;
  }

  export enum TerminationReason {
    Timeout = 0x80,
    InvalidHandshake,
    InvalidToken,
    InvalidDeliveryAcknowledgment,
    InvalidService,
    ServiceTerminated,
    ConnectionInUse,
    WrongPermssion,
    Unknown = 0xFF,
  }

  export function encode(frame: Frame): ArrayBuffer {
    if (frame.tag === Tag.ServicePayload) {
      const u8 = new Uint8Array(frame.bytes.byteLength + 1);
      u8[0] = 0x00;
      u8.set(frame.bytes, 1);
      return u8.buffer;
    }

    if (frame.tag === Tag.ChunkedServicePayload) {
      const u8 = new Uint8Array(frame.bytes.byteLength + 1);
      u8[0] = 0x40;
      u8.set(frame.bytes, 1);
      return u8.buffer;
    }

    if (frame.tag === Tag.AccessToken) {
      const writer = new Writer(57);
      writer.putU8(0x01);
      writer.putU64(frame.ttl);
      writer.put(frame.accessToken);
      return writer.getBuffer();
    }

    if (frame.tag === Tag.Termination) {
      const u8 = new Uint8Array(1);
      u8[0] = frame.reason;
      return u8.buffer;
    }

    throw new Error("Unsupported");
  }

  export function decode(payload: ArrayBuffer): Frame | undefined {
    const reader = new Reader(payload);
    const tag = reader.getU8();

    if (tag == 0x00) {
      return {
        tag: Tag.ServicePayload,
        bytes: new Uint8Array(payload).slice(1),
      };
    }

    if (tag == 0x40) {
      return {
        tag: Tag.ChunkedServicePayload,
        bytes: new Uint8Array(payload).slice(1),
      };
    }

    if (tag == 0x01) {
      const ttl = reader.getU64();
      const accessToken = reader.get(64) as RawAccessToken;
      return {
        tag: Tag.AccessToken,
        ttl,
        accessToken,
      };
    }

    if (tag < 0x80) {
      throw new Error("Unsupported");
    }

    return {
      tag: Tag.Termination,
      reason: {
        0: TerminationReason.Timeout,
        1: TerminationReason.InvalidHandshake,
        2: TerminationReason.InvalidToken,
        3: TerminationReason.InvalidDeliveryAcknowledgment,
        4: TerminationReason.InvalidService,
        5: TerminationReason.ServiceTerminated,
        6: TerminationReason.ConnectionInUse,
        7: TerminationReason.WrongPermssion,
      }[tag - 0x80] || TerminationReason.Unknown,
    };
  }
}

export class Writer {
  private readonly buffer: Uint8Array;
  private readonly view: DataView;
  private cursor = 0;

  constructor(size: number) {
    this.buffer = new Uint8Array(size);
    this.view = new DataView(this.buffer.buffer);
  }

  putU8(n: number) {
    this.view.setUint8(this.cursor, n);
    this.cursor += 1;
  }

  putU32(n: number) {
    this.view.setUint32(this.cursor, n);
    this.cursor += 4;
  }

  putU64(n: number) {
    const lo = n & 0xffffffff;
    const hi = n >> 4;
    this.view.setUint32(this.cursor, hi);
    this.view.setUint32(this.cursor + 4, lo);
    this.cursor += 8;
  }

  put(array: ArrayLike<number>) {
    this.buffer.set(array, this.cursor);
    this.cursor += array.length;
  }

  getBuffer(): ArrayBuffer {
    if (this.cursor != this.buffer.byteLength) {
      console.warn("dead allocation");
      return this.buffer.buffer.slice(0, this.cursor);
    }

    return this.buffer.buffer;
  }
}

export class Reader {
  private readonly buffer: Uint8Array;
  private readonly view: DataView;
  private cursor = 0;

  constructor(buffer: ArrayBuffer) {
    this.buffer = new Uint8Array(buffer);
    this.view = new DataView(buffer);
  }

  getU8(): number {
    return this.buffer[this.cursor++];
  }

  getU32(): number {
    const offset = this.cursor;
    this.cursor += 4;
    return this.view.getUint32(offset);
  }

  getU64(): number {
    const offset = this.cursor;
    this.cursor += 8;
    const hi = this.view.getUint32(offset);
    const lo = this.view.getUint32(offset + 4);
    // TODO(qti3e): Handle overflow
    return (hi << 4) + lo;
  }

  get(length: number): Uint8Array {
    const offset = this.cursor;
    this.cursor += length;
    return this.buffer.slice(offset, this.cursor);
  }
}
