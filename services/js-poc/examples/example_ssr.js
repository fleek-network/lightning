// Example simple ssr script for testing ideas for a browser request
// curl localhost:4220/services/1/blake3/$(lightning-node dev store services/js-poc/examples/example_ssr.js | awk '{print $1}')

export function main({ path, method, headers, body }) {
  switch (path) {
    case '/index.html':
      return {
        status: 200,
        body: '<html><body><h1>Hello World!</h1></body></html>',
        headers: { "x-custom": "foobar" },
      }
    default:
      return {
        status: 404,
        body: '<html><body><p>404 not found</p></body></html>',
        headers: [
          ["x-custom", "foobar"],
          ["x-original", arguments[0]]
        ],
      }
  }
}
