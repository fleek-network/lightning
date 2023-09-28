import * as schema from "../handshake/schema.ts";

const video = document.querySelector("video")!;

document.getElementById("start")!.onclick = async () => {
  await startSession();
};

// State variables.

const queue: Uint8Array[] = []; // Because MediaStream sucks.
let didReceiveFirstMessage = false;
let sourceBuffer: SourceBuffer | undefined;
let segCounter = 0;
let bytesRead = 0;
let getNextSent: undefined | number;

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
    service: 1 as schema.ServiceId,
    pk: new Uint8Array(96) as schema.ClientPublicKey,
    pop: new Uint8Array(48) as schema.ClientSignature,
  }));
};

dataChan.onmessage = (e: MessageEvent) => {
  if (!didReceiveFirstMessage) {
    didReceiveFirstMessage = true;
    if (sourceBuffer != undefined) {
      getNext();
    }
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
    if (bytesRead == 0) {
      console.log(performance.measure("first-byte", {
        start: getNextSent,
        end: performance.now(),
      }));
    }

    bytesRead += decoded.bytes.byteLength;
    appendBuffer(decoded.bytes);

    if (bytesRead >= 256 * 1024) {
      getNext();
      bytesRead = 0;
    }
  }
};

function appendBuffer(buffer: Uint8Array) {
  if (sourceBuffer!.updating || queue.length != 0) {
    queue.push(buffer);
    return;
  }

  if (getNextSent != undefined) {
    console.log(performance.measure("first-frame", {
      start: getNextSent,
      end: performance.now(),
    }));
    getNextSent = undefined;
  }

  sourceBuffer!.appendBuffer(buffer);
}

function getNext() {
  getNextSent = performance.now();
  const buffer = new ArrayBuffer(4);
  const view = new DataView(buffer);
  view.setUint32(0, segCounter++);

  dataChan.send(schema.Request.encode({
    tag: schema.Request.Tag.ServicePayload,
    bytes: new Uint8Array(buffer),
  }));
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

  const res = await fetch("http://localhost:4210/sdp", {
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

  console.log("got sd: ", sd);

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
