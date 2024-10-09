import * as loc from "ext:deno_web/12_location.js";
import { globalContext } from "ext:fleek/global.js";

/** Bootstrap function called at runtime before execution.
 *  Can only be called once.
 *  @param {number} time - Timestamp to hardcode to Date.now()
 */
globalThis.bootstrap = (time, url) => {
  // Define webapis in the global scope
  Object.defineProperties(globalThis, globalContext);

  // Hardcode timestamp
  globalThis.Date.now = () => time;

  // Set runtime location
  loc.setLocationHref(url);

  // Block internal access to deno from the script scope
  delete globalThis.Deno;
};
