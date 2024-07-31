export const main = async () => {
  const request = {
    origin: "Ipfs",
    uri: "bafkreievqw7zbdxko2fm5vsk7mzmnqaxkuxfnbukjndul5apoumssbimqa",
  };

  // Run the task locally
  const { responses } = await Fleek.runTask(Fleek.ServiceId.Js, request);

  // Run the task on a single other node
  // const { responses } = await Fleek.runTask(Fleek.ServiceId.Js, request, "single");

  // Run the task on the current node's cluster
  // const { responses } = await Fleek.runTask(Fleek.ServiceId.Js, request, "cluster");

  if (responses.length == 0) {
    return "error: failed to run task";
  } else {
    console.log(responses);
    // Return the first response's payload directly
    return new Uint8Array(responses[0])
  }
}
