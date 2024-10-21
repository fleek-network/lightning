import { core } from "ext:core/mod.js";
const { ops } = core;

/** Service Ids */
const ServiceId = {
  Fetcher: 0 | 0,
  Js: 1 | 0,
  Ai: 2 | 0,
  Sgx: 3 | 0,
}

/** Run a service task either locally, on a single node in the cluster, or replicated across the cluster.
 * @param {Number} service - Service ID, must be a 32 bit unsigned integer
 * @param {ArrayBufferLike | string | any} body - Request body to send to a service. Buffers are sent directly,
 *                                                strings are encoded, and anything else is encoded as json.
 * @param {"local"|"single"|"cluster"} scope - Optional scope to run the task under. If undefined, defaults to local.
 * @returns {Promise<Uint8Array>} - Raw response body from the service
 */
const runTask = async (service, body, scope = "local") => {
  // TODO: move this encoding to rust
  if (!ArrayBuffer.isView(body)) {
    let encoder = new TextEncoder();
    if (typeof body == "string") {
      body = encoder.encode(body);
    } else {
      body = encoder.encode(JSON.stringify(body));;
    }
  }
  return await ops.run_task(service, body, scope)
};

/** Fetch some blake3 content
 * @param {Uint8Array} hash - Blake3 hash of content to fetch
 * @returns {Promise<bool>} True if the fetch was successful
 */
const fetchBlake3 = async (hash) => await ops.fetch_blake3(hash);

/** Fetch some content from an origin and return the b3 hash
 * @param {String} url - Origin url for the content to fetch.
 *                       Ie, `ipfs://bafy...` or  `https://...#integrity=sha256-...`
 * @returns {Promise<Uint8Array>} True if the fetch was successful
 */
const fetchFromOrigin = async (url) => await ops.fetch_from_origin(url);

/** Load a blockstore handle to some blake3 content
 * @param {Uint8Array} hash - Blake3 hash of the content
 * @returns {Promise<ContentHandle>}
 */
const loadContent = async (hash) => {
  const proof = await ops.load_content(hash);
  return new ContentHandle(proof);
};

/** Fetch a clients FLK balance.
 * @param {Uint8Array} account - The balance to check
 * @returns {Promise<BigInt>} BigInt of the balance
 */
const queryClientFlkBalance = async (account) => {
  const balance = await ops.query_client_flk_balance(account);
  return BigInt(balance, 10);
};

/** Fetch a clients FLK balance.
 * @param {Uint8Array} account - The balance to check
 * @returns {Promise<BigInt>} BigInt of the balance
 */
const queryClientBandwidthBalance = async (account) => {
  const balance = await ops.query_client_bandwidth_balance(account);
  return BigInt(balance, 10);
};

/** Handle to blockstore content.
 * Utility for traversing the proof and reading blocks from the blockstore.
 * @property {Uint8Array} proof - Blake3 proof of the content
 * @property {number} length - Number of blocks in the content
 */
class ContentHandle {
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

  /**
    * Read a given block index from the blockstore
    * @param {number} idx - Index of block to read
    * @returns {Promise<Uint8Array>}
    */
  readBlock(idx) {
    return ops.read_block(this.proof, idx);
  }
}

/**
  * Fleek API namespace
  */
export const Fleek = {
  ContentHandle,
  ServiceId,
  runTask,
  fetchBlake3,
  fetchFromOrigin,
  loadContent,
  queryClientFlkBalance,
  queryClientBandwidthBalance,
};
