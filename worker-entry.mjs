// Unified worker entry point
// Combines the Rust/WASM fetch handler with the JS email handler
import WorkerClass from './build/worker/shim.mjs';
import emailShim from './src/email_shim.js';

// Re-export everything from the built worker
export * from './build/worker/shim.mjs';

// Create a subclass that adds the email handler to the WorkerEntrypoint class
class UnifiedWorker extends WorkerClass {
  async email(message) {
    return emailShim.email(message, this.env, this.ctx);
  }
}

export default UnifiedWorker;
