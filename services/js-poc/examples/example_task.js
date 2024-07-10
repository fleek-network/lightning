export const main = async () => {
  // Encode request for a hello world function
  const encoder = new TextEncoder();
  let body = encoder.encode(JSON.stringify({
    origin: "Ipfs",
    uri: "bafkreievqw7zbdxko2fm5vsk7mzmnqaxkuxfnbukjndul5apoumssbimqa",
  }));

  // Run the task locally
  const { responses } = await Fleek.runTask(Fleek.ServiceId.Js, body);

  // Return the response directly
  return new Uint8Array(responses[0])
}
