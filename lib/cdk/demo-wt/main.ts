import * as schema from "../handshake/schema.ts";

const video = document.querySelector("video")!;

document.getElementById("start")!.onclick = async () => {
  await startSession();
};

const transport = new WebTransport("url");

let readStream: ReadableStream;
let writeStream: WritableStream;
let sourceBuffer: SourceBuffer | undefined;
let readBuffer: Uint8Array = new Uint8Array(256 * 1024 + 4);
const queue: Uint8Array[] = [];

const handshake = async () => {
  // Make connection.
  await transport.ready;
  // Open a bi-directional stream.
  const stream = await transport.createBidirectionalStream();

  readStream = stream.readable;
  writeStream = stream.writable;

  await send(
    schema.HandshakeRequest.encode({
      tag: schema.HandshakeRequest.Tag.Handshake,
      service: 1 as schema.ServiceId,
      pk: new Uint8Array(96) as schema.ClientPublicKey,
      pop: new Uint8Array(48) as schema.ClientSignature,
    }),
  );
};

const send = async (buffer: ArrayBuffer) => {
  if (!writeStream) {
  }
  const length = buffer.byteLength;
  const view = new DataView(buffer);
  view.setUint32(0, length, false);
  const writer = writeStream.getWriter();
  writer.write(view);
};

const recv = async () => {
  const reader = readStream.getReader();

  // Read the length header.
  while (readBuffer.length < 4) {
    let { value, done } = await reader.read();
    readBuffer.set(value, readBuffer.length);
    if (done) {
      console.error("reader ended unexpectedly");
      return;
    }
  }

  const view = new DataView(readBuffer.buffer);
  const lengthPrefix = view.getUint32(0, false);
  readBuffer = readBuffer.slice(4);

  // Read the frame.
  while (readBuffer.length < lengthPrefix) {
    const { value, done } = await reader.read();
    readBuffer.set(value, readBuffer.length);
    if (done) {
      console.error("reader ended unexpectedly");
      return;
    }
  }

  const frame = schema.Response.decode(readBuffer.slice(0, lengthPrefix));
  readBuffer = readBuffer.slice(lengthPrefix);
  return frame;
};

function appendBuffer(buffer: Uint8Array) {
  if (sourceBuffer!.updating || queue.length != 0) {
    queue.push(buffer);
    return;
  }
  sourceBuffer!.appendBuffer(buffer);
}

// Todo: Handle errors and log.
const startSession = async () => {
  await handshake();

  // Send a request for the CID.
  const buffer = new ArrayBuffer(4);
  await send(schema.Request.encode({
    tag: schema.Request.Tag.ServicePayload,
    bytes: new Uint8Array(buffer),
  }));

  // Read the number of blocks we should receive back from the first frame.
  let frame = await recv();

  if (!frame || frame.tag !== schema.Response.Tag.ServicePayload) {
    console.error("invalid tag");
    return;
  }

  appendBuffer(frame.bytes);

  const view = new DataView(frame.bytes.buffer);
  const blockCount = view.getUint32(0, false);
  for (let i = 0; i < blockCount; i++) {
    let frame = await recv();
    if (!frame || frame.tag !== schema.Response.Tag.ServicePayload) {
      console.error("invalid tag");
      return;
    }

    appendBuffer(frame.bytes);
  }
};

// --------------------------------------------
// MediaSource

// Need to be specific for Blink regarding codecs
// ./mp4info frag_bunny.mp4 | grep Codec
const mimeCodec = 'video/mp4; codecs="avc1.42E01E, mp4a.40.2"';

if ("MediaSource" in window && MediaSource.isTypeSupported(mimeCodec)) {
  const mediaSource = new MediaSource();
  //console.log(mediaSource.readyState); // closed
  video.src = URL.createObjectURL(mediaSource);
  mediaSource.addEventListener("sourceopen", sourceOpen);
} else {
  console.error("Unsupported MIME type or codec: ", mimeCodec);
}

function sourceOpen(this: MediaSource) {
  console.log(this.readyState); // open
  sourceBuffer = this.addSourceBuffer(mimeCodec);

  sourceBuffer!.addEventListener("updateend", function (_) {
    if (queue.length != 0) {
      sourceBuffer!.appendBuffer(
        queue.shift()!,
      );
    }
  });
}
