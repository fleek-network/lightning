function notImplemented() {
  throw new Error("Not implemented");
}

module.exports = fs = {
  appendFile: notImplemented,
  appendFileSync: notImplemented,
  access: notImplemented,
  accessSync: notImplemented,
  chown: notImplemented,
  chownSync: notImplemented,
  chmod: notImplemented,
  chmodSync: notImplemented,
  close: notImplemented,
  closeSync: notImplemented,
  copyFile: notImplemented,
  copyFileSync: notImplemented,
  cp: notImplemented,
  cpSync: notImplemented,
  createReadStream: notImplemented,
  createWriteStream: notImplemented,
  exists: notImplemented,
  existsSync: notImplemented,
  fchown: notImplemented,
  fchownSync: notImplemented,
  fchmod: notImplemented,
  fchmodSync: notImplemented,
  fdatasync: notImplemented,
  fdatasyncSync: notImplemented,
  fstat: notImplemented,
  fstatSync: notImplemented,
  fsync: notImplemented,
  fsyncSync: notImplemented,
  ftruncate: notImplemented,
  ftruncateSync: notImplemented,
  futimes: notImplemented,
  futimesSync: notImplemented,
  glob: notImplemented,
  globSync: notImplemented,
  lchown: notImplemented,
  lchownSync: notImplemented,
  lchmod: notImplemented,
  lchmodSync: notImplemented,
  link: notImplemented,
  linkSync: notImplemented,
  lstat: notImplemented,
  lstatSync: notImplemented,
  lutimes: notImplemented,
  lutimesSync: notImplemented,
  mkdir: notImplemented,
  mkdirSync: notImplemented,
  mkdtemp: notImplemented,
  mkdtempSync: notImplemented,
  open: notImplemented,
  openSync: notImplemented,
  openAsBlob: notImplemented,
  readdir: notImplemented,
  readdirSync: notImplemented,
  read: notImplemented,
  readSync: notImplemented,
  readv: notImplemented,
  readvSync: notImplemented,
  readFile: notImplemented,
  readFileSync: notImplemented,
  readlink: notImplemented,
  readlinkSync: notImplemented,
  realpath: notImplemented,
  realpathSync: notImplemented,
  rename: notImplemented,
  renameSync: notImplemented,
  rm: notImplemented,
  rmSync: notImplemented,
  rmdir: notImplemented,
  rmdirSync: notImplemented,
  stat: notImplemented,
  statfs: notImplemented,
  statSync: notImplemented,
  statfsSync: notImplemented,
  symlink: notImplemented,
  symlinkSync: notImplemented,
  truncate: notImplemented,
  truncateSync: notImplemented,
  unwatchFile: notImplemented,
  unlink: notImplemented,
  unlinkSync: notImplemented,
  utimes: notImplemented,
  utimesSync: notImplemented,
  watch: notImplemented,
  watchFile: notImplemented,
  writeFile: notImplemented,
  writeFileSync: notImplemented,
  write: notImplemented,
  writeSync: notImplemented,
  writev: notImplemented,
  writevSync: notImplemented,
  Dirent,
  Stats,

  get ReadStream() {
    return notImplemented();
  },

  set ReadStream(val) {
    notImplemented();
  },

  get WriteStream() {
    return notImplemented();
  },

  set WriteStream(val) {
    notImplemented();
  },

  // Legacy names... these have to be separate because of how graceful-fs
  // (and possibly other) modules monkey patch the values.
  get FileReadStream() {
    return notImplemented();
  },

  set FileReadStream(val) {
    notImplemented();
  },

  get FileWriteStream() {
    return notImplemented();
  },

  set FileWriteStream(val) {
    notImplemented();
  },

  // For tests
  _toUnixTimestamp: notImplemented,
};
