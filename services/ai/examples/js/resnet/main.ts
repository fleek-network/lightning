import { CLASSES } from "./imagenet.ts";
import { deserialize, serialize } from "https://esm.sh/borsh@2.0.0";
import { decode, encode } from "https://deno.land/x/msgpack@v1.2/mod.ts";
import { ExtData } from "https://deno.land/x/msgpack@v1.4/ExtData.ts";
import { ExtensionCodec } from "https://deno.land/x/msgpack@v1.4/mod.ts";

const inputElement = document.querySelector("input");
const outputElement = document.querySelector("output");

const BorshUint8Encoding: number = 4;

interface Output {
  squeezed: ExtData;
}

inputElement!.addEventListener("change", async () => {
  if (inputElement!.files && inputElement!.files.length > 0) {
    const serializedImage = await inputElement!.files[0].arrayBuffer();
    const extensionCodec = ExtensionCodec.defaultCodec;
    const inputSchema = { array: { type: "u8" } };
    const serializedInput = serialize(
      inputSchema,
      new Uint8Array(serializedImage),
    );
    const body = encode({
      array: {
        data: new ExtData(BorshUint8Encoding, new Uint8Array(serializedInput)),
      },
    }, { extensionCodec });

    const resp = await fetch(
      "http://127.0.0.1:4220/services/2/blake3/c0a9b26955bf5175802624d94f07e6d87d844d3d29aadbe11902ec0830a30e37",
      {
        method: "POST",
        headers: {
          accept: "application/json",
        },
        body: body,
      },
    );

    const payload = await resp.arrayBuffer();
    const output = decode(new Uint8Array(payload)) as Output;
    const encodedData = output.squeezed.data;

    const outputSchema = { array: { type: "f32" } };
    const deserializedOutput = deserialize(outputSchema, encodedData);

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
