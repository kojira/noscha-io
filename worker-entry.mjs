// Unified worker entry point
// Combines the Rust/WASM fetch handler with the JS email handler
import Worker from './build/worker/shim.mjs';
import emailShim from './src/email_shim.js';

export * from './build/worker/shim.mjs';

Worker.prototype.email = emailShim.email;
export default Worker;
