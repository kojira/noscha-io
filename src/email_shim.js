// Email event handler - called by Cloudflare Email Routing
// Uses message.forward() for direct email forwarding
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

    const forwardTo = rental.services.email.forward_to;

    // Forward email directly via Cloudflare Email Routing
    await message.forward(forwardTo);
  }
};
