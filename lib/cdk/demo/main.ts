import * as schema from "../handshake/schema.ts";

console.log(schema);

console.log("Hello World");

// For webRTC:
// 1. We need to make a RTCPeerConnection with the ice server we want. Should be the same
//  as the node is using.
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

const dataChan = pc.createDataChannel("foo");

dataChan.onclose = () => {
  console.log("dataChan closed.");
};

dataChan.onopen = () => {
  console.log("dataChan opened.");
};

dataChan.onmessage = (e: MessageEvent) => {
  console.log("dataChan message", e);
};

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

document.getElementById("start")!.onclick = async () => {
  await startSession();
};
