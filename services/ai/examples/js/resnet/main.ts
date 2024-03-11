import { CLASSES } from "./imagenet.ts";
import { deserialize, serialize } from "https://esm.sh/borsh@2.0.0";
import {
  decodeBase64,
  encodeBase64,
} from "https://deno.land/std@0.218.2/encoding/base64.ts";

const inputElement = document.querySelector("input");
const outputElement = document.querySelector("output");

inputElement!.addEventListener("change", async () => {
  if (inputElement!.files && inputElement!.files.length > 0) {
    const image = await inputElement!.files[0].arrayBuffer();
    const inputSchema = { array: { type: "u8" } };
    const serializedInput = serialize(
      inputSchema,
      new Uint8Array(image),
    );

    const body = JSON.stringify({
      "image": {
        dtype: "u8",
        data: encodeBase64(serializedInput),
      },
    });

    const resp = await fetch(
      "http://127.0.0.1:4220/services/2/infer/blake3/c0a9b26955bf5175802624d94f07e6d87d844d3d29aadbe11902ec0830a30e37",
      {
        method: "POST",
        headers: {
          accept: "application/json",
        },
        body: body,
      },
    );

    const response = await resp.json();
    const serializedVector = decodeBase64(response.squeezed.data);
    const outputSchema = { array: { type: "f32" } };
    const deserializedOutput = deserialize(outputSchema, serializedVector);

    if (deserializedOutput) {
      const logits = new Float32Array(deserializedOutput as ArrayBuffer);
      const bestPrediction = logits.indexOf(Math.max(...logits));
      const name = CLASSES[bestPrediction];

      if (outputElement) {
        outputElement.innerHTML = `<div class="image"><img src="${
          URL.createObjectURL(inputElement!.files[0])
        }" alt="image"></div><div>${name}</div>`;
      }
    }
  }
});
