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

/** Normalize charset to TextDecoder label (shift-jis, euc-jp, iso-2022-jp, utf-8) */
function normalizeCharset(charset) {
  const c = (charset || 'utf-8').toLowerCase().replace(/_/g, '-');
  const map = {
    'shift-jis': 'shift-jis', 'shift_jis': 'shift-jis', 'sjis': 'shift-jis', 'x-sjis': 'shift-jis', 'csshiftjis': 'shift-jis',
    'euc-jp': 'euc-jp', 'euc_jp': 'euc-jp', 'x-euc-jp': 'euc-jp', 'cseucpkdfmtjapanese': 'euc-jp',
    'iso-2022-jp': 'iso-2022-jp', 'iso2022jp': 'iso-2022-jp', 'csiso2022jp': 'iso-2022-jp'
  };
  return map[c] || c;
}

/**
 * Decode RFC 2047 encoded-words in header values (e.g. Subject).
 * Format: =?charset?B?base64?= or =?charset?Q?quoted-printable?=
 * Supports: UTF-8, Shift-JIS, EUC-JP, ISO-2022-JP
 */
function decodeRFC2047(str) {
  if (!str || typeof str !== 'string') return str || '';
  return str.replace(/=\?([^?]*)\?([BQbq])\?([^?]*)\?=/g, (full, charset, enc, payload) => {
    try {
      const c = normalizeCharset(charset);
      if (enc.toUpperCase() === 'B') {
        return decodeBase64UTF8(payload.replace(/\s/g, ''), c);
      }
      if (enc.toUpperCase() === 'Q') {
        const decoded = payload.replace(/_/g, ' ').replace(/=([0-9A-Fa-f]{2})/g, (_, hex) => String.fromCharCode(parseInt(hex, 16)));
        const bytes = new Uint8Array([...decoded].map((ch) => ch.charCodeAt(0)));
        return new TextDecoder(c).decode(bytes);
      }
    } catch (_) {}
    return full;
  });
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

// Debug webhook: Discord embed colors by level (decimal)
const DEBUG_COLORS = { debug: 0x9e9e9e, info: 0x5865f2, warn: 0xfee75c, error: 0xed4245 };
const LEVEL_PRIORITY = { off: 0, error: 1, warn: 2, info: 3, debug: 4 };

async function loadDebugWebhookConfig(env) {
  try {
    const obj = await env.BUCKET.get("config/debug_webhook.json");
    if (!obj) return { enabled: false, webhook_url: "", level: "off" };
    const cfg = JSON.parse(await obj.text());
    return {
      enabled: !!cfg.enabled,
      webhook_url: (cfg.webhook_url || "").trim(),
      level: (cfg.level || "off").toLowerCase()
    };
  } catch {
    return { enabled: false, webhook_url: "", level: "off" };
  }
}

async function debugLog(cfg, level, stage, msg, data = {}) {
  if (!cfg.enabled || !cfg.webhook_url) return;
  const cfgPriority = LEVEL_PRIORITY[cfg.level] ?? 0;
  const logPriority = LEVEL_PRIORITY[level] ?? 0;
  // Send when log level is at or "above" in severity (e.g. config=info sends info,warn,error)
  if (logPriority > cfgPriority) return;

  const fields = [
    { name: "Level", value: level, inline: true },
    { name: "Stage", value: stage, inline: true }
  ];
  for (const [k, v] of Object.entries(data)) {
    const val = typeof v === "object" ? "```json\n" + JSON.stringify(v, null, 0).slice(0, 900) + "\n```" : String(v).slice(0, 1024);
    fields.push({ name: k, value: val || "-", inline: k.length <= 10 });
  }

  const embed = {
    title: "noscha.io Email Worker",
    description: `[${level.toUpperCase()}] ${msg}`,
    color: DEBUG_COLORS[level] ?? 0x9e9e9e,
    fields,
    timestamp: new Date().toISOString(),
    footer: { text: "noscha.io debug" }
  };

  try {
    const ctrl = new AbortController();
    const timeout = setTimeout(() => ctrl.abort(), 10000);
    await fetch(cfg.webhook_url, {
      method: "POST",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify({ embeds: [embed] }),
      signal: ctrl.signal
    });
    clearTimeout(timeout);
  } catch (e) {
    console.error(`Debug webhook send failed: ${e.message}`);
  }
}

// Email event handler - called by Cloudflare Email Routing
// Handles incoming emails via webhook notification
export default {
  async email(message, env, ctx) {
    const recipient = message.to;
    const username = recipient.split("@")[0].toLowerCase();
    const cfg = await loadDebugWebhookConfig(env);

    await debugLog(cfg, "debug", "email_received_start", `To: ${recipient}`, { to: recipient, username });

    // Lookup rental from R2
    const key = `rentals/${username}.json`;
    const obj = await env.BUCKET.get(key);

    if (!obj) {
      await debugLog(cfg, "warn", "rental_not_found", "Address not found", { username });
      message.setReject("Address not found");
      return;
    }

    const rental = JSON.parse(await obj.text());

    if (rental.status !== "active") {
      await debugLog(cfg, "warn", "status_not_active", "Address expired", { username, status: rental.status });
      message.setReject("Address expired");
      return;
    }

    if (!rental.services?.email?.enabled) {
      await debugLog(cfg, "warn", "email_not_enabled", "Email service not enabled", { username });
      message.setReject("Email service not enabled");
      return;
    }

    // Check expiry
    const now = new Date();
    const expires = new Date(rental.expires_at);
    if (now > expires) {
      await debugLog(cfg, "warn", "expired", "Address expired", { username, expires_at: rental.expires_at });
      message.setReject("Address expired");
      return;
    }

    // Read email body and check size limit (256KB)
    const rawEmail = await new Response(message.raw).text();
    const MAX_EMAIL_SIZE = 256 * 1024; // 256KB

    if (rawEmail.length > MAX_EMAIL_SIZE) {
      await debugLog(cfg, "warn", "email_too_large", "Email too large (256KB limit)", { username, size: rawEmail.length });
      message.setReject("Email too large (256KB limit)");
      return;
    }

    // Extract subject from raw email headers (handle folding per RFC 5322), decode RFC 2047
    const subjectMatch = rawEmail.match(/^Subject:\s*([^\r\n]*(?:\r?\n[\t ][^\r\n]*)*)/im);
    let subject = subjectMatch ? subjectMatch[1].replace(/\r?\n[\t ]/g, ' ').trim() : '(no subject)';
    subject = decodeRFC2047(subject);

    // Extract Date header from raw email
    const dateMatch = rawEmail.match(/^Date:\s*(.+)$/mi);
    const emailDate = dateMatch ? dateMatch[1].trim() : null;

    // Extract both text and HTML bodies
    const bodies = extractBodies(rawEmail);
    const bodyText = bodies.text || extractBody(rawEmail); // fallback to old function

    // Generate mail ID and save to R2
    const mailId = crypto.randomUUID().split("-")[0];
    const emailData = {
      from: message.from,
      to: recipient,
      subject: subject,
      body_text: bodies.text,
      body_html: bodies.html,
      date: emailDate,
      username: username,
      mail_id: mailId,
      created_at: now.toISOString(),
      read_at: null
    };

    try {
      await env.BUCKET.put(`inbox/${username}/${mailId}.json`, JSON.stringify(emailData));
    } catch (e) {
      await debugLog(cfg, "error", "r2_save_failed", `Failed to save email: ${e.message}`, { username, mail_id: mailId });
      console.error(`Failed to save email to inbox: ${e.message}`);
      message.setReject("Email processing failed");
      return;
    }

    await debugLog(cfg, "info", "email_saved", "Email saved to R2", { username, mail_id: mailId, from: message.from, subject });

    // Check if rental has webhook_url - if so, send notification
    if (rental.webhook_url) {
      try {
        const viewUrl = `https://${env.DOMAIN || "noscha.io"}/api/mail/${username}/${mailId}`;
        const isDiscord = /discord\.com\/api\/webhooks|discordapp\.com\/api\/webhooks/i.test(rental.webhook_url);

        let body;
        if (isDiscord) {
          body = JSON.stringify({
            embeds: [{
              title: "📧 New email",
              description: `**From:** ${message.from}\n**Subject:** ${subject}`,
              url: viewUrl,
              color: 0x5865f2,
              fields: [
                { name: "To", value: recipient, inline: true },
                { name: "View", value: `[Open](${viewUrl})`, inline: true },
                { name: "Received", value: now.toISOString(), inline: false }
              ],
              timestamp: now.toISOString(),
              footer: { text: "noscha.io" }
            }]
          });
        } else {
          body = JSON.stringify({
            event: "email_received",
            username,
            mail_id: mailId,
            from: message.from,
            to: recipient,
            subject,
            date: now.toISOString(),
            view_url: viewUrl,
            received_at: now.toISOString()
          });
        }

        const webhookRes = await fetch(rental.webhook_url, {
          method: "POST",
          headers: { "Content-Type": "application/json" },
          body,
        });

        if (!webhookRes.ok) {
          await debugLog(cfg, "error", "webhook_failed", `Webhook notification failed: ${webhookRes.status}`, { username, status: webhookRes.status });
          console.error(`Webhook notification failed: ${webhookRes.status}`);
        } else {
          await debugLog(cfg, "info", "webhook_sent", "Webhook notification sent", { username, mail_id: mailId });
        }
      } catch (e) {
        await debugLog(cfg, "error", "webhook_failed", `Webhook notification error: ${e.message}`, { username });
        console.error(`Webhook notification error: ${e.message}`);
      }

      return; // Done - webhook notified
    }

    await debugLog(cfg, "warn", "no_webhook_configured", "No webhook configured for this address", { username });
    message.setReject("No webhook configured for this address");
  }
};
