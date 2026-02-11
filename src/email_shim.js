// Parse email body from raw email, stripping headers and handling multipart/encoding
function extractBody(rawEmail) {
  // Split headers and body at first blank line
  const splitIdx = rawEmail.indexOf('\r\n\r\n');
  const splitIdx2 = rawEmail.indexOf('\n\n');
  let headerPart, bodyPart;
  if (splitIdx !== -1 && (splitIdx2 === -1 || splitIdx < splitIdx2)) {
    headerPart = rawEmail.substring(0, splitIdx);
    bodyPart = rawEmail.substring(splitIdx + 4);
  } else if (splitIdx2 !== -1) {
    headerPart = rawEmail.substring(0, splitIdx2);
    bodyPart = rawEmail.substring(splitIdx2 + 2);
  } else {
    return rawEmail; // no headers found, return as-is
  }

  // Check Content-Type for multipart
  const ctMatch = headerPart.match(/^Content-Type:\s*(.+?)(?:\r?\n(?=\s)|\r?\n|$)/mi);
  const contentType = ctMatch ? ctMatch[1] : '';

  // Get charset from Content-Type
  const charsetMatch = contentType.match(/charset="?([^";\s]+)"?/i);
  const charset = charsetMatch ? charsetMatch[1].toLowerCase() : 'utf-8';

  // Get Content-Transfer-Encoding
  const cteMatch = headerPart.match(/^Content-Transfer-Encoding:\s*(\S+)/mi);
  const encoding = cteMatch ? cteMatch[1].toLowerCase() : '7bit';

  if (/multipart\//i.test(contentType)) {
    return extractMultipartBody(contentType, bodyPart);
  }

  return decodeBody(bodyPart, encoding, charset);
}

function extractMultipartBody(contentType, body) {
  // Extract boundary
  const bMatch = contentType.match(/boundary="?([^";\s]+)"?/i);
  if (!bMatch) return body;
  const boundary = bMatch[1];

  const parts = body.split('--' + boundary);
  let htmlPart = null;
  let textPart = null;

  for (const part of parts) {
    if (part.trim() === '' || part.trim() === '--') continue;

    // Split part headers from part body
    const pSplit = part.indexOf('\r\n\r\n');
    const pSplit2 = part.indexOf('\n\n');
    let pHeaders, pBody;
    if (pSplit !== -1 && (pSplit2 === -1 || pSplit < pSplit2)) {
      pHeaders = part.substring(0, pSplit);
      pBody = part.substring(pSplit + 4);
    } else if (pSplit2 !== -1) {
      pHeaders = part.substring(0, pSplit2);
      pBody = part.substring(pSplit2 + 2);
    } else {
      continue;
    }

    const pCtMatch = pHeaders.match(/Content-Type:\s*([^;\r\n]+)/i);
    const pCt = pCtMatch ? pCtMatch[1].trim().toLowerCase() : '';
    const pCtFull = pHeaders.match(/Content-Type:\s*(.+?)(?:\r?\n(?=\s)|\r?\n|$)/mi);
    const pCtFullStr = pCtFull ? pCtFull[1] : '';
    const pCharsetMatch = pCtFullStr.match(/charset="?([^";\s]+)"?/i);
    const pCharset = pCharsetMatch ? pCharsetMatch[1].toLowerCase() : 'utf-8';
    const pCteMatch = pHeaders.match(/Content-Transfer-Encoding:\s*(\S+)/i);
    const pEnc = pCteMatch ? pCteMatch[1].toLowerCase() : '7bit';

    // Recurse into nested multipart
    if (pCt.startsWith('multipart/')) {
      const fullCt = pHeaders.match(/Content-Type:\s*(.+?)(?:\r?\n(?=\s)|\r?\n|$)/mi);
      const nested = extractMultipartBody(fullCt ? fullCt[1] : pCt, pBody);
      if (nested) return nested;
      continue;
    }

    if (pCt === 'text/html') {
      htmlPart = decodeBody(pBody, pEnc, pCharset);
    } else if (pCt === 'text/plain') {
      textPart = decodeBody(pBody, pEnc, pCharset);
    }
  }

  // Prefer text/plain for our <pre> display; html would need sanitizing
  return textPart || htmlPart || body;
}

function decodeBase64UTF8(base64str, charset = 'utf-8') {
  const binaryStr = atob(base64str);
  const bytes = new Uint8Array(binaryStr.length);
  for (let i = 0; i < binaryStr.length; i++) {
    bytes[i] = binaryStr.charCodeAt(i);
  }
  return new TextDecoder(charset).decode(bytes);
}

function decodeBody(body, encoding, charset = 'utf-8') {
  // Remove trailing boundary artifacts
  body = body.replace(/--[\w=+/]+--\s*$/, '').trim();

  if (encoding === 'base64') {
    try {
      return decodeBase64UTF8(body.replace(/\s/g, ''), charset);
    } catch { return body; }
  }
  if (encoding === 'quoted-printable') {
    const raw = body
      .replace(/=\r?\n/g, '') // soft line breaks
      .replace(/=([0-9A-Fa-f]{2})/g, (_, hex) => String.fromCharCode(parseInt(hex, 16)));
    // Decode as UTF-8 (or specified charset) from binary string
    try {
      const bytes = new Uint8Array(raw.length);
      for (let i = 0; i < raw.length; i++) {
        bytes[i] = raw.charCodeAt(i);
      }
      return new TextDecoder(charset).decode(bytes);
    } catch { return raw; }
  }
  return body;
}

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

    // Extract Date header from raw email
    const dateMatch = rawEmail.match(/^Date:\s*(.+)$/mi);
    const emailDate = dateMatch ? dateMatch[1].trim() : null;

    // Parse email: separate headers from body at first blank line
    const bodyText = extractBody(rawEmail);

    // Build forwarded email via Resend API
    const apiKey = env.RESEND_API_KEY;
    if (!apiKey) {
      message.setReject("Email forwarding temporarily unavailable");
      return;
    }

    const fromAddr = `noreply@noscha.io`;
    const forwardTo = rental.services.email.forward_to;

    const escapedBody = bodyText.replace(/</g, '&lt;').replace(/>/g, '&gt;');
    const htmlBody = `<div style="font-family:sans-serif;color:#333">
<p style="color:#888;font-size:12px;border-bottom:1px solid #eee;padding-bottom:8px;margin-bottom:12px">
Forwarded from <strong>${message.from}</strong> to <strong>${recipient}</strong> via noscha.io${emailDate ? ` | Date: ${emailDate}` : ''}
</p>
<pre style="white-space:pre-wrap;font-family:inherit">${escapedBody}</pre>
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
