/** Log a message when service is built on debug mode
 * @param {string} msg - the message to print
 */
const log = (msg) => Deno.core.ops.log(msg);

/** Fetch some blake3 content 
 * @param {Uint8Array} hash - Blake3 hash of content to fetch 
 * @returns {Promise<bool>} True if the fetch was successful
 */
const fetch_blake3 = async (hash) => Deno.core.ops.fetch_blake3(hash);

/** Load a blockstore handle to some blake3 content
 * @param {Uint8Array} hash - Blake3 hash of the content
 * @returns {Promise<ContentHandle>}
 */
const load_content = async (hash) => {
  const proof = await Deno.core.ops.load_content(hash);
  return new ContentHandle(proof);
}

/**Handle to blockstore content.
 * Traverses the proof and reads blocks from the blockstore.
 * @class
 * @property {Uint8Array} proof - Blake3 proof of the content
 * @property {number} length - Number of blocks in the content
 */
export class ContentHandle {
  proof;
  length;

  /** 
   * @constructor
   * @param {Uint8Array} proof - Blake3 proof of some content
   * @returns {ContentHandle}
   */
  constructor(proof) {
    this.proof = proof;
    const num_hashes = this.proof.length / 32 | 0;
    this.length = ((num_hashes + 1) >> 1) | 0;
  }

  read(id) {
    return Deno.core.ops.read_block(this.proof, id);
  }
}

globalThis.Fleek = {
  log,
  fetch_blake3,
  load_content,
};
