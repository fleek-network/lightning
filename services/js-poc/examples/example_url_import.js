// Library import from ipfs
import * as Blake3 from 'ipfs://bafkreidlknwg33twrtlm5tagbpdyhkovouzkpkp2sfpp4n2i6o4jiclq5i';

// node:path polyfill
import path from 'node:path';

// json import using subresource integrity
import data from
  'https://rickandmortyapi.com/api/character/781#integrity=sha256-tZfoN2TwZclgq4/eBDLAcrZGjor0JN8iI92XdRNKcyg=' with
  { type: 'json' };

function encodeHex(buffer) {
  return [...new Uint8Array(buffer)].map((x) => x.toString(16).padStart(2, "0")).join("");
}

export async function main() {
  // Download the image from the data, and hash it with blake3.js
  // (I know, it's silly, but it's just an example)
  const url = data.image;

  const image = await fetch(url).then((res) => res.arrayBuffer());
  const hash = Blake3.hash(new Uint8Array(image));

  // use a node polyfill to do something
  const basename = path.basename(url);

  return `url: ${url}\nbasename: ${basename}\nblake3 hash: ${encodeHex(hash)}`;
}
