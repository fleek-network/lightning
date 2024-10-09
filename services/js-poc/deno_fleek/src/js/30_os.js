
function getExitCode() {
    throw new Error("Unsupported");
}

function setExitCode(value) {
    throw new Error("Unsupported");
}

function osUptime() {
    throw new Error("Unsupported");
}


export {
    getExitCode,
    osUptime,
    setExitCode,
};