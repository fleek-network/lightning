function serve(arg1, arg2) {
    throw new Error("Unsupported"); // Same as Deno deploy
}


function upgradeHttpRaw(req, conn) {
    throw new Error("Unsupported");
}


/**
 * Serve HTTP/1.1 and/or HTTP/2 on an arbitrary connection.
 */
function serveHttpOnConnection(connection, signal, handler, onError, onListen) {
    throw new Error("Unsupported"); // Same as Deno deploy
}

export {
    serve,
    serveHttpOnConnection,
    upgradeHttpRaw,
};