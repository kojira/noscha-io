// Email event handler - called by Cloudflare Email Routing
// Uses Resend API to forward emails instead of message.forward()
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

    // Read email body
    const rawEmail = await new Response(message.raw).text();

    // Extract subject from raw email headers
    const subjectMatch = rawEmail.match(/^Subject:\s*(.+)$/mi);
    const subject = subjectMatch ? subjectMatch[1].trim() : "(no subject)";

    // Build forwarded email via Resend API
    const apiKey = env.RESEND_API_KEY;
    if (!apiKey) {
      message.setReject("Email forwarding temporarily unavailable");
      return;
    }

    const fromAddr = `noreply@noscha.io`;
    const forwardTo = rental.services.email.forward_to;

    const htmlBody = `<div style="font-family:sans-serif;color:#333">
<p style="color:#888;font-size:12px;border-bottom:1px solid #eee;padding-bottom:8px;margin-bottom:12px">
Forwarded from <strong>${message.from}</strong> to <strong>${recipient}</strong> via noscha.io
</p>
<pre style="white-space:pre-wrap;font-family:inherit">${rawEmail.replace(/</g, '&lt;').replace(/>/g, '&gt;')}</pre>
</div>`;

    const res = await fetch("https://api.resend.com/emails", {
      method: "POST",
      headers: {
        "Authorization": `Bearer ${apiKey}`,
        "Content-Type": "application/json",
      },
      body: JSON.stringify({
        from: fromAddr,
        to: [forwardTo],
        subject: `[noscha.io] ${subject}`,
        html: htmlBody,
      }),
    });

    if (!res.ok) {
      const errText = await res.text();
      console.error(`Resend API error: ${res.status} ${errText}`);
      message.setReject("Email forwarding failed");
      return;
    }
  }
};
