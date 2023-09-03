/// <reference types="./types.d.ts" />

/**
 * A 32-byte hash.
 */
export type Digest = Opaque<"Digest", Uint8Array>;

/**
 * A numeric connection id. In Rust we use u64. Which may overflow here.
 *
 * TODO(qti3e): Figure this out. What's the state of bigints now?
 */
export type ConnectionId = Opaque<"ConnectionId", number>;

export type ServiceId = Opaque<"ServiceId", number>;

export type ClientPublicKey = Opaque<"ClientPublicKey", Uint8Array>;

export type ClientSignature = Opaque<"ClientSignature", Uint8Array>;

export type NodeSignature = Opaque<"NodeSignature", Uint8Array>;

export type NodePublicKey = Opaque<"NodePublicKey", Uint8Array>;

export type AccessToken = Opaque<"AccessToken", Uint8Array> & { length: 32 };

export interface ChallengeFrame {
  challenge: Digest;
}

export type HandshakeRequestFrame =
  | HandshakeRequest
  | JoinRequest;

export enum HandshakeRequestFrameTag {
  Handshake,
  JoinRequest,
}

export interface HandshakeRequest {
  readonly tag: HandshakeRequestFrameTag.Handshake;
  retry?: ConnectionId;
  service: ServiceId;
  pk: ClientPublicKey;
  pop: ClientSignature;
}

export interface JoinRequest {
  readonly tag: HandshakeRequestFrameTag.Handshake;
  access_token: AccessToken;
}

export interface HandshakeResponseFrame {
  pk: NodePublicKey;
  pop: NodeSignature;
}

export type RequestFrame =
  | ServicePayloadRequest
  | AccessTokenRequest
  | ExtendAccessTokenRequest;

export enum RequestFrameTag {
  ServicePayload,
  AccessToken,
  ExtendAccessToken,
}

export interface ServicePayloadRequest {
  readonly tag: RequestFrameTag.ServicePayload;
  bytes: ArrayBuffer;
}

export interface AccessTokenRequest {
  readonly tag: RequestFrameTag.AccessToken;
  ttl: number;
}

export interface ExtendAccessTokenRequest {
  readonly tag: RequestFrameTag.ExtendAccessToken;
  ttl: number;
}

export type ResponseFrame =
  | ServicePayloadResponse
  | AccessTokenResponse
  | TerminationResponse;

export enum ResponseFrameTag {
  ServicePayload,
  AccessToken,
  Termination,
}

export interface ServicePayloadResponse {
  readonly tag: ResponseFrameTag.ServicePayload;
  bytes: ArrayBuffer;
}

export interface AccessTokenResponse {
  readonly tag: ResponseFrameTag.AccessToken;
  access_token: AccessToken;
}

export interface TerminationResponse {
  readonly tag: ResponseFrameTag.Termination;
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
