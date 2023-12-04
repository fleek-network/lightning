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

// TODO: once ipfs/unixfs content is properly supported, we can use this again
// const bbb_cid = new Uint8Array([
//     1, 112,  18,  32,  40, 237,  86,  26, 103, 105, 148, 233,
//    41, 174, 225, 186, 191,  54,  94,  88,  74,  99, 112,  43,
//   212, 151,  49,   8, 123,  58,  25, 118, 155,  32, 104, 178
// ]);

// State variables.
const queue: Uint8Array[] = []; // Because MediaStream sucks.
let sourceBuffer: SourceBuffer | undefined;
let bytesRead = 0;
let receivedBlockCount = false;

// --------------------------------------------
// WebRTC
//
// For webRTC:
// 1. We need to make a RTCPeerConnection with the ice server we have.
// 2. We then create a data channel over the RTCPeerConnection
//    This will get the messages from the node, and other events.
// 3. We send /sdp request to a node on click and start the negotiation with the node.

const pc = new RTCPeerConnection({
  iceServers: [
    {
      urls: "stun:stun.l.google.com:19302",
    },
  ],
});

const dataChan = pc.createDataChannel("fleek", {
  ordered: true,
});

dataChan.onclose = () => {
  console.log("dataChan closed.");
};

dataChan.onopen = () => {
  console.log("dataChan opened.");

  // Send a handshake.
  dataChan.send(schema.HandshakeRequest.encode({
    tag: schema.HandshakeRequest.Tag.Handshake,
    service: 0 as schema.ServiceId,
    pk: new Uint8Array(96) as schema.ClientPublicKey,
    pop: new Uint8Array(48) as schema.ClientSignature,
  }));

  console.log("sent handshake");

  const buffer = new Uint8Array(33);
  buffer[0] = 0; // Blake3 Origin
  buffer.set(bbb_blake3, 1); // UID

  // Send the request
  dataChan.send(schema.Request.encode({
    tag: schema.Request.Tag.ServicePayload,
    bytes: buffer,
  }));

  console.log("sent request");
};

dataChan.onmessage = (e: MessageEvent) => {
  // handle first frame
  if (!receivedBlockCount) {
    console.log("got block count");
    receivedBlockCount = true;
    return;
  }

  const decoded = schema.Response.decode(e.data);
  if (
    decoded &&
    (decoded.tag ===
        schema.Response.Tag.ServicePayload ||
      decoded.tag ===
        schema.Response.Tag.ChunkedServicePayload)
  ) {
    bytesRead += decoded.bytes.length;
    appendBuffer(decoded.bytes);
  }
};

function appendBuffer(buffer: Uint8Array) {
  if (queue.length != 0 || sourceBuffer!.updating) {
    queue.push(buffer);
  } else {
    sourceBuffer!.appendBuffer(buffer);
  }
}

pc.oniceconnectionstatechange = () => {
  console.log("pc status: ", pc.iceConnectionState);
};

pc.onicecandidate = (e) => {
  if (e.candidate === null) {
    console.log("pc description: ", pc.localDescription);
  }
};

pc.onnegotiationneeded = async (e) => {
  try {
    console.log("connection negotiation ended", e);
    const d = await pc.createOffer();
    pc.setLocalDescription(d);
  } catch (e) {
    console.error("error: ", e);
  }
};

const startSession = async () => {
  console.log("sending sdp signal");

  const res = await fetch("http://localhost:4220/sdp", {
    method: "POST",
    headers: {
      "Content-Type": "application/json",
    },
    body: JSON.stringify(pc.localDescription),
  });

  const sd = await res.json();

  if (sd === "") {
    return alert("Session Description must not be empty");
  }

  console.log("got sdp response: ", sd);

  try {
    pc.setRemoteDescription(new RTCSessionDescription(sd));
  } catch (e) {
    alert(e);
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
