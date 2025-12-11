/**
 * Custom version updater for tauri.conf.json
 * Used by standard-version to update the version in Tauri config
 */

const fs = require('fs');

module.exports.readVersion = function (contents) {
  const config = JSON.parse(contents);
  return config.version;
};

module.exports.writeVersion = function (contents, version) {
  const config = JSON.parse(contents);
  config.version = version;
  return JSON.stringify(config, null, 2) + '\n';
};

