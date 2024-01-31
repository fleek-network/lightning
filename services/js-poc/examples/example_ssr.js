// Example simple ssr script for testing ideas for a browser request
// cargo run --example js-poc-client $(lgtn-old dev store services/js-poc/examples/example_ssr.js | awk '{print $1}') blake3 '{"path":"/index.html"}'

const main = (params = {}) => {
  console.log(params);

  let path = params.path || '';

  switch (path) {
    case '/index.html':
      return '<html><body><h1>Hello World!</h1></body></html>'
    default:
      return '<html><body><p>404 not found</p></body></html>'
  }
}
