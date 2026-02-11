// Unified worker entry point
// Combines the Rust/WASM fetch handler with the JS email handler
import fetchHandler from './build/worker/shim.mjs';
import emailShim from './src/email_shim.js';

export * from './build/worker/shim.mjs';

export default {
  fetch: fetchHandler.fetch || fetchHandler.prototype?.fetch,
  email: emailShim.email,
  scheduled: fetchHandler.scheduled || fetchHandler.prototype?.scheduled,
};
