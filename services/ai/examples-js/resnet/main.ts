import { CLASSES } from "./imagenet.ts";

const input = document.querySelector("input");
const output = document.querySelector("output");

interface SessionOutput {
  format: string;
  outputs: Outputs;
}

interface Outputs {
  [outputs: string]: Float32Array;
}

input!.addEventListener("change", async () => {
  if (input!.files && input!.files.length > 0) {
    const resp = await fetch(
      "http://127.0.0.1:4220/services/2/blake3/f2700c0d695006d953ca920b7eb73602b5aef7dbe7b6506d296528f13ebf0d95",
      {
        method: "POST",
        headers: {
          accept: "application/json",
        },
        body: await input!.files[0].arrayBuffer(),
      },
    );

    const results: SessionOutput = await resp.json();
    const array = results.outputs["output"];
    const bestPrediction = array.indexOf(Math.max(...array));
    const name = CLASSES[bestPrediction];

    if (output) {
      output.innerHTML = `<div class="image"><img src="${
        URL.createObjectURL(input!.files[0])
      }" alt="image"></div><div>${name}</div>`;
    }
  }
});
