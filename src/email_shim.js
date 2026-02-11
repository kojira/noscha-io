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

// Extract both text and HTML bodies from raw email
function extractBodies(rawEmail) {
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
    return { text: rawEmail, html: null }; // no headers found, return as-is
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
    return extractMultipartBodies(contentType, bodyPart);
  }

  const decoded = decodeBody(bodyPart, encoding, charset);
  // Check if content type indicates HTML
  if (/text\/html/i.test(contentType)) {
    return { text: null, html: decoded };
  }
  return { text: decoded, html: null };
}

function extractMultipartBodies(contentType, body) {
  // Extract boundary
  const bMatch = contentType.match(/boundary="?([^";\s]+)"?/i);
  if (!bMatch) return { text: body, html: null };
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
      const nested = extractMultipartBodies(fullCt ? fullCt[1] : pCt, pBody);
      if (nested.text && !textPart) textPart = nested.text;
      if (nested.html && !htmlPart) htmlPart = nested.html;
      continue;
    }

    if (pCt === 'text/html') {
      htmlPart = decodeBody(pBody, pEnc, pCharset);
    } else if (pCt === 'text/plain') {
      textPart = decodeBody(pBody, pEnc, pCharset);
    }
  }

  return { text: textPart, html: htmlPart };
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

    if (!rental.services?.email?.enabled) {
      message.setReject("Email service not enabled");
      return;
    }

    // Check expiry
    const now = new Date();
    const expires = new Date(rental.expires_at);
    if (now > expires) {
      message.setReject("Address expired");
      return;
    }

    // Read email body and check size limit (256KB)
    const rawEmail = await new Response(message.raw).text();
    const MAX_EMAIL_SIZE = 256 * 1024; // 256KB

    if (rawEmail.length > MAX_EMAIL_SIZE) {
      message.setReject("Email too large (256KB limit)");
      return;
    }

    // Rate limit: 5 emails per 24 hours per user
    const DAILY_LIMIT = 5;
    const WINDOW_MS = 24 * 60 * 60 * 1000;
    const countKey = `emails/${username}.json`;
    let emailCount = { count: 0, window_start: now.toISOString() };
    try {
      const countObj = await env.BUCKET.get(countKey);
      if (countObj) {
        emailCount = JSON.parse(await countObj.text());
        const windowStart = new Date(emailCount.window_start);
        if (now - windowStart >= WINDOW_MS) {
          // Window expired, reset
          emailCount = { count: 0, window_start: now.toISOString() };
        }
      }
    } catch (e) {
      console.error(`Rate limit check error: ${e.message}`);
    }

    if (emailCount.count >= DAILY_LIMIT) {
      message.setReject("Daily email forwarding limit reached (5/day)");
      return;
    }

    // Extract subject from raw email headers
    const subjectMatch = rawEmail.match(/^Subject:\s*(.+)$/mi);
    const subject = subjectMatch ? subjectMatch[1].trim() : "(no subject)";

    // Extract Date header from raw email
    const dateMatch = rawEmail.match(/^Date:\s*(.+)$/mi);
    const emailDate = dateMatch ? dateMatch[1].trim() : null;

    // Extract both text and HTML bodies
    const bodies = extractBodies(rawEmail);
    const bodyText = bodies.text || extractBody(rawEmail); // fallback to old function

    // Generate random token and save to R2
    const randomToken = crypto.randomUUID();
    const emailData = {
      from: message.from,
      to: recipient,
      subject: subject,
      body_text: bodies.text,
      body_html: bodies.html,
      date: emailDate,
      username: username,
      random_token: randomToken,
      created_at: now.toISOString(),
      read_at: null
    };

    try {
      await env.BUCKET.put(`inbox/${randomToken}.json`, JSON.stringify(emailData));
    } catch (e) {
      console.error(`Failed to save email to inbox: ${e.message}`);
      message.setReject("Email processing failed");
      return;
    }

    // Check if rental has webhook_url - if so, send notification and skip Resend forwarding
    if (rental.webhook_url) {
      try {
        const webhookPayload = {
          event: "email_received",
          from: message.from,
          to: recipient,
          subject: subject,
          url: `https://noscha.io/api/mail/${randomToken}`,
          received_at: now.toISOString()
        };

        const webhookRes = await fetch(rental.webhook_url, {
          method: "POST",
          headers: {
            "Content-Type": "application/json",
          },
          body: JSON.stringify(webhookPayload),
        });

        if (!webhookRes.ok) {
          console.error(`Webhook notification failed: ${webhookRes.status}`);
        }
      } catch (e) {
        console.error(`Webhook notification error: ${e.message}`);
      }

      // Increment email count after successful webhook notification
      emailCount.count += 1;
      try {
        await env.BUCKET.put(countKey, JSON.stringify(emailCount));
      } catch (e) {
        console.error(`Failed to update email count: ${e.message}`);
      }
      return; // Done - webhook notified
    }

    // No webhook_url configured - reject
    message.setReject("No webhook configured for this address");
  }
};
