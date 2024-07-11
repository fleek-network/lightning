export const main = async () => {
  // Run the task locally
  const { responses } = await Fleek.runTask(Fleek.ServiceId.Js, {
    origin: "Ipfs",
    uri: "bafkreievqw7zbdxko2fm5vsk7mzmnqaxkuxfnbukjndul5apoumssbimqa",
  });

  // Return the response directly
  return new Uint8Array(responses[0])
}
