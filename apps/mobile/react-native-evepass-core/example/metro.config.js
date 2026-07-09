const path = require('path');
const { getDefaultConfig } = require('@react-native/metro-config');
const { withMetroConfig } = require('react-native-monorepo-config');

const root = path.resolve(__dirname, '..');

/**
 * Metro configuration
 * https://facebook.github.io/metro/docs/configuration
 *
 * @type {import('metro-config').MetroConfig}
 */
const config = withMetroConfig(getDefaultConfig(__dirname), {
  root,
  dirname: __dirname,
  conditions: ['react-native-evepass-core-source'],
});

// The UBRN-generated bindings (src/generated/evepass_core.ts) import the runtime
// as the bare specifier `@ubjs/core`. That package is installed nested under the
// library (node_modules/uniffi-bindgen-react-native/typescript), so Metro can't
// resolve it on its own — alias it explicitly.
config.resolver.extraNodeModules = {
  ...(config.resolver.extraNodeModules || {}),
  '@ubjs/core': path.resolve(
    root,
    'node_modules/uniffi-bindgen-react-native/typescript',
  ),
};

module.exports = config;
