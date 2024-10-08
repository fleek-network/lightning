export const main = async () => {
  const wasmHash =
    await Fleek.fetchFromOrigin("ipfs://bafkreibun52plarbynribunkylscurinox3y6z2cs5g6ydelymj4zpdu4i")
      .then(bytes => Array.from(bytes).map(v => v.toString(16).padStart(2, '0')).join('')); // convert to hex string

  const request = {
    hash: wasmHash,
    decrypt: false,
    input: "hello world from echo wasm!"
  };

  // Run the task locally
  const { responses } = await Fleek.runTask(Fleek.ServiceId.Sgx, request);

  if (responses.length == 0) {
    return "error: failed to run sgx task";
  } else {
    return new Uint8Array(responses[0])
  }
}
