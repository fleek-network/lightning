// Hello world example
// curl localhost:4220/services/1/blake3/$(lightning-node dev store services/js-poc/examples/example.js | awk '{print $1}')

class Deferred {
  resolve;
  reject;
  inner;
  constructor() {
    this.inner = new Promise((resolve, reject) => {
      this.resolve = resolve;
      this.reject = reject;
    })
  }
}

export const main = async () => {
  let promise = new Deferred();

  let socket = new WebSocket("wss://echo.websocket.org");
  socket.onopen = function(_) {
    console.log("opened socket, sending message...");
    socket.send("hello world");
  };
  socket.onmessage = function(event) {
    console.log("got message: " + event.data);
    socket.close();
  };
  socket.onclose = function(_) {
    console.log("socket closed");
    promise.resolve();
  };
  socket.onerror = function(_) {
    console.log("socket error");
    promise.reject();
  }

  await promise.inner;
};
