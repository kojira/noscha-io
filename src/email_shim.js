// Email event handler - called by Cloudflare Email Routing
// This JS shim bridges CF Email Workers API to our Rust/WASM logic
// worker-rs 0.7 does NOT support #[event(email)], so this shim handles it directly in JS.
export default {
  async email(message, env, ctx) {
    const recipient = message.to;
    const username = recipient.split("@")[0].toLowerCase();

    // Lookup rental from R2
    const key = `rentals/${username}.json`;
    const obj = await env.BUCKET.get(key);

    if (!obj) {
      message.setReject("Address not found");
      return;
    }

    const rental = JSON.parse(await obj.text());

    if (rental.status !== "active") {
      message.setReject("Address expired");
      return;
    }

    if (!rental.services?.email?.enabled || !rental.services?.email?.forward_to) {
      message.setReject("Email forwarding not configured");
      return;
    }

    // Check expiry
    const now = new Date();
    const expires = new Date(rental.expires_at);
    if (now > expires) {
      message.setReject("Address expired");
      return;
    }

    // Forward the email
    await message.forward(rental.services.email.forward_to);
  }
};
