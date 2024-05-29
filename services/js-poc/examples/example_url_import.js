// Library import from ipfs
import * as Blake3 from 'ipfs://bafkreidlknwg33twrtlm5tagbpdyhkovouzkpkp2sfpp4n2i6o4jiclq5i';

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

  console.log(data);

  const url = data.image;
  const image = await (await fetch(url)).arrayBuffer();
  const hash = Blake3.hash(new Uint8Array(image));

  return `url: ${url}\nblake3 hash: ${encodeHex(hash)}`;
}
