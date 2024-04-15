// Example simple ssr script for testing ideas for a browser request
// curl localhost:4220/services/1/blake3/$(lightning-node dev store services/js-poc/examples/example_ssr.js | awk '{print $1}')

export const main = () => {
  const path = window.location.pathname;

  switch (path) {
    case '/index.html':
      return '<html><body><h1>Hello World!</h1></body></html>'
    default:
      return '<html><body><p>404 not found</p></body></html>'
  }
}
