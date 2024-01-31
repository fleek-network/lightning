// Example script for fetching some content with the sdk
// cargo run --example js-poc-client $(lgtn-old dev store services/js-poc/examples/example_fetch.js | awk '{print $1}') blake3

const bbb_hash = new Uint8Array([
  16,
  101,
  178,
  253,
  130,
  145,
  238,
  45,
  55,
  180,
  144,
  250,
  71,
  121,
  27,
  31,
  201,
  144,
  67,
  224,
  179,
  36,
  52,
  86,
  242,
  33,
  164,
  55,
  27,
  140,
  43,
  209,
]);

const main = async () => {
  console.log("Hello world from javascript!");

  if (await Fleek.fetch_blake3(bbb_hash)) {
    console.log("Content fetched");

    const handle = await Fleek.load_content(bbb_hash);
    console.log(`Loaded content handle with ${handle.length} blocks`);

    let total = 0;
    for (let i = 0; i < handle.length; i++) {
      const block = await handle.read(0);
      console.log(`Read block ${i}: ${block.length}`);
      total += block.length;
    }

    return { success: true, blocks: handle.length, bytes: total };
  } else {
    return { success: false };
  }
};
