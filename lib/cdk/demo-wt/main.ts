import * as schema from "../handshake/schema.ts";

const video = document.querySelector("video")!;

document.getElementById("start")!.onclick = async () => {
  await startSession();
};

// Temporary internal hash for big buck bunny
const bbb_blake3 = new Uint8Array([
  16,
  101,
  178,
  253,
  130,
  145,
  238,
  45,
  55,
  180,
  144,
  250,
  71,
  121,
  27,
  31,
  201,
  144,
  67,
  224,
  179,
  36,
  52,
  86,
  242,
  33,
  164,
  55,
  27,
  140,
  43,
  209,
]);

let transport: WebTransport;
let readStream: ReadableStream;
let writeStream: WritableStream;
let writer: WritableStreamDefaultWriter;
let sourceBuffer: SourceBuffer | undefined;
let readBuffer: Uint8Array = new Uint8Array(256 * 1024 + 4);
const queue: Uint8Array[] = [];

const handshake = async () => {
  // Request the certificate hash for the node
  const res = await fetch("http://127.0.0.1:4220/certificate-hash");
  const hash = await res.arrayBuffer();

  // Make connection.
  transport = new WebTransport("https://127.0.0.1:4321", {
    serverCertificateHashes: [{ algorithm: "sha-256", value: hash }],
  });
  await transport.ready;

  // Open a bi-directional stream.
  const stream = await transport.createBidirectionalStream();

  readStream = stream.readable;
  writeStream = stream.writable;
  writer = writeStream.getWriter();

  send(
    schema.HandshakeRequest.encode({
      tag: schema.HandshakeRequest.Tag.Handshake,
      service: 0 as schema.ServiceId,
      pk: new Uint8Array(96) as schema.ClientPublicKey,
      pop: new Uint8Array(48) as schema.ClientSignature,
    }),
  );
};

const send = (buffer: ArrayBuffer) => {
  if (!writeStream) {
    console.log("failed to write to stream");
  }

  const length = buffer.byteLength;
  const delim = new DataView(new ArrayBuffer(4));
  delim.setUint32(0, length, false);

  writer.write(delim);
  writer.write(buffer);
};

const recv = async () => {
  const reader = readStream.getReader();

  // Read the length header.
  while (readBuffer.length < 4) {
    const { value, done } = await reader.read();
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
  const reqWriter = new schema.Writer(bbb_blake3.length + 1);
  reqWriter.putU8(0); // Blake3 origin id
  reqWriter.put(bbb_blake3);
  send(schema.Request.encode({
    tag: schema.Request.Tag.ServicePayload,
    bytes: new Uint8Array(reqWriter.getBuffer()),
  }));

  // Read the number of blocks we should receive back from the first frame.
  const frame = await recv();
  if (!frame || frame.tag !== schema.Response.Tag.ServicePayload) {
    console.error("invalid tag");
    return;
  }
  const view = new DataView(frame.bytes.buffer);
  const blockCount = view.getUint32(0, false);

  // Read each block from the stream
  for (let i = 0; i < blockCount; i++) {
    const frame = await recv();
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
