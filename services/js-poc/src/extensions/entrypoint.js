globalThis.Fleek = {
  "log": Deno.core.ops.log,
  "fetch_blake3": Deno.core.ops.fetch_blake3,
  "load_content": async (root) => {
    const proof = await Deno.core.ops.load_content(root);
    return new ContentHandle(proof);
  },
};

export class ContentHandle {
  proof;
  length;

  constructor(proof) {
    this.proof = proof;
    const num_hashes = this.proof.length / 32 | 0;
    this.length = ((num_hashes + 1) >> 1) | 0;
  }

  read(id) {
    return Deno.core.ops.read_block(this.proof, id);
  }
}
