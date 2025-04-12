const ENCRYPTED_B64_DATA = "BOAm0XAK37qNHGRKUb9an6JCy8S7u2wSHmnFG3TyjMMvrqVnYsema8slOMg+wnqPaNAPdQ0ia1g1ZJaRLXFwYuXZvwZfpzyQS3r+lMypvqsXcQ9hKFl9kYmuKb1Ew4+Uv18XTi5XAOzlxOCWjqdiSEqOlDqoWKpvk+eZedM7ig==";
const DECRYPTION_WASM = "40f0e2f81be0bc5b28d10e97165bc50bea8c79478a0092582c94f80f05001222";
const DECRYPTION_WASM_CID = "bafkreibp4psloudnwv7rfyq6t6ar6gtw6ethrxh5kdw4yciwjh2qlpbpvu";

export async function main(params) {
  // const { path, body } = params;

  // ensure we have the wasm module locally
  await Fleek.fetchFromOrigin(`ipfs://${DECRYPTION_WASM_CID}`);

  // build sgx request
  const request = {
    type: "shared",
    data: ENCRYPTED_B64_DATA,
  };

  // decrypt the secret
  const response = await Fleek.runTask(Fleek.ServiceId.Sgx, {
    hash: DECRYPTION_WASM,
    decrypt: false,
    input: JSON.stringify(request),
  });

  console.log(response);

  if (response.responses.length == 0) {
    return "failed to decrypt secrets";
  } else {
    return String.fromCharCode(response.responses[0], null);
  }
}
