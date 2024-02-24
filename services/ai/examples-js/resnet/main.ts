import { CLASSES } from "./imagenet.ts";
import { deserialize } from "https://esm.sh/borsh@2.0.0";
import { decode, encode } from "https://deno.land/x/msgpack@v1.2/mod.ts";
import { ExtData } from "https://deno.land/x/msgpack@v1.4/ExtData.ts";
import { ExtensionCodec } from "https://deno.land/x/msgpack@v1.4/mod.ts";

const inputElement = document.querySelector("input");
const outputElement = document.querySelector("output");

const RawEncoding: number = 0;

interface Output {
  outputs: {
    squeezed: ExtData;
  };
}

inputElement!.addEventListener("change", async () => {
  if (inputElement!.files && inputElement!.files.length > 0) {
    const serializedImage = await inputElement!.files[0].arrayBuffer();
    const extensionCodec = ExtensionCodec.defaultCodec;
    const body = encode({
      array: {
        data: new ExtData(RawEncoding, new Uint8Array(serializedImage)),
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
    const encodedData = output.outputs.squeezed.data;

    const schema = { array: { type: "f32" } };
    const decoded = deserialize(schema, encodedData);

    if (decoded) {
      const logits = new Float32Array(decoded as ArrayBuffer);
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
