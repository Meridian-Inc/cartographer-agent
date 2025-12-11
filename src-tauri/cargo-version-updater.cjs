/**
 * Custom version updater for Cargo.toml
 * Used by standard-version to update the version in Rust config
 */

module.exports.readVersion = function (contents) {
  const match = contents.match(/^\s*version\s*=\s*"([^"]+)"/m);
  if (match) {
    return match[1];
  }
  throw new Error('Could not find version in Cargo.toml');
};

module.exports.writeVersion = function (contents, version) {
  return contents.replace(
    /^(\s*version\s*=\s*)"[^"]+"/m,
    `$1"${version}"`
  );
};

