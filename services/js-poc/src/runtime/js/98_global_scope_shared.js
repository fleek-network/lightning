import { globalContext } from "ext:fleek/global.js";

export const windowOrWorkerGlobalScope = {
    console: globalContext.console
};