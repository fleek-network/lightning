// deno-lint-ignore-file no-namespace
/// <reference types="./types.d.ts" />

export type Digest = Opaque<"Digest", Uint8Array>;

export type ConnectionId = Opaque<"ConnectionId", number>;

export type ServiceId = Opaque<"ServiceId", number>;

export type ClientPublicKey = Opaque<"ClientPublicKey", Uint8Array>;

export type ClientSignature = Opaque<"ClientSignature", Uint8Array>;

export type NodeSignature = Opaque<"NodeSignature", Uint8Array>;

export type NodePublicKey = Opaque<"NodePublicKey", Uint8Array>;

export type AccessToken = Opaque<"AccessToken", Uint8Array>;

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
    access_token: AccessToken;
  }

  export function encode(frame: Frame): ArrayBuffer {
    if (frame.tag === Tag.JoinRequest) {
      const u8 = new Uint8Array(49);
      u8[0] = 2;
      u8.set(frame.access_token, 1);
      return u8.buffer;
    } else if (frame.retry === undefined) {
      const buffer = new ArrayBuffer(149);
      const view = new DataView(buffer);
      const u8 = new Uint8Array(buffer);
      u8[0] = 0x00;
      view.setUint32(1, frame.service);
      u8.set(frame.pk, 2);
      u8.set(frame.pop, 98);
      return buffer;
    } else {
      const buffer = new ArrayBuffer(157);
      const view = new DataView(buffer);
      const u8 = new Uint8Array(buffer);
      u8[0] = 0x01;
      write_u64(frame.retry, view, 1);
      view.setInt32(9, frame.service);
      u8.set(frame.pk, 10);
      u8.set(frame.pop, 106);
      return buffer;
    }
  }

  export function decode(payload: ArrayBuffer): Frame | undefined {
    throw new Error("todo");
  }
}

export namespace HandshakeResponse {
  export interface Frame {
    pk: NodePublicKey;
    pop: NodeSignature;
  }

  export function encode(frame: Frame): ArrayBuffer {
    throw new Error("TODO");
  }

  export function decode(payload: ArrayBuffer): Frame | undefined {
    return undefined;
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
    bytes: ArrayBuffer;
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
    throw new Error("TODO");
  }

  export function decode(payload: ArrayBuffer): Frame | undefined {
    return undefined;
  }
}

export namespace Response {
  export type Frame =
    | ServicePayload
    | AccessToken
    | Termination;

  export enum Tag {
    ServicePayload,
    AccessToken,
    Termination,
  }

  export interface ServicePayload {
    readonly tag: Tag.ServicePayload;
    bytes: ArrayBuffer;
  }

  export interface AccessToken {
    readonly tag: Tag.AccessToken;
    access_token: AccessToken;
  }

  export interface Termination {
    readonly tag: Tag.Termination;
    reason: TerminationReason;
  }

  export enum TerminationReason {
    Timeout = 0x00,
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
    throw new Error("TODO");
  }

  export function decode(payload: ArrayBuffer): Frame | undefined {
    return undefined;
  }
}

function write_u64(value: number, array: DataView, offset: number) {
  const hi = value >> 4;
  const lo = value & 0xffff;
  array.setUint32(offset, hi);
  array.setUint32(offset + 4, lo);
}

function _read_u64(array: DataView, offset: number): number {
  const hi = array.getUint32(offset);
  const lo = array.getUint32(offset + 4);
  return (hi << 4) + lo;
}
