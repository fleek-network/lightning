import { CLASSES } from "./imagenet.ts";
import {
  deserialize, serialize
} from "https://deno.land/x/mongo@v0.31.0/deps.ts";

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
    const serializedImage = await input!.files[0].arrayBuffer()
    const body = serialize({
      "type": "array",
      "encoding": "raw",
      "data": new Uint8Array(serializedImage),
    });
    const resp = await fetch(
      "http://127.0.0.1:4220/services/2/blake3/f2700c0d695006d953ca920b7eb73602b5aef7dbe7b6506d296528f13ebf0d95",
      {
        method: "POST",
        headers: {
          accept: "application/json",
        },
        body: body,
      },
    );

    const payload = await resp.arrayBuffer();
    const bson = deserialize(new Uint8Array(payload));
    const logits = bson.outputs.logits;
    const bestPrediction = logits.indexOf(Math.max(...logits));
    const name = CLASSES[bestPrediction];

    if (output) {
      output.innerHTML = `<div class="image"><img src="${
        URL.createObjectURL(input!.files[0])
      }" alt="image"></div><div>${name}</div>`;
    }
  }
});
