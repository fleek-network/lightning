// import * as loc from "ext:deno_web/12_location.js";
import { globalContext } from 'ext:fleek/global.js';

/** Bootstrap function called at runtime before execution.
 *  Can only be called once.
 *  @param {number} time - Timestamp to hardcode to Date.now()
 */
globalThis.bootstrap = (time) => {
  // Define webapis in the global scope
  Object.defineProperties(globalThis, globalContext);

  // Hardcode to timestamp passed to the bootstrap
  globalThis.Date.now = () => time;

  // WebIDL thing
  // globalThis[webidl.brand] = webidl.brand;

  // globalThis.location = new URL("http://example.com"); 

  // Block internal access to deno from the script scope
  delete Deno.core;
  delete Deno.internal;
};
