// Unified worker entry point
// Combines the Rust/WASM fetch handler with the JS email handler
import WorkerClass from './build/worker/shim.mjs';
import emailShim from './src/email_shim.js';

// Re-export everything from the built worker
export * from './build/worker/shim.mjs';

/**
 * NIP-07 auth gate for staging environments.
 * When REQUIRE_AUTH=true, all pages (except /health and /api/*) require
 * a valid NIP-07 signed event proving the user holds ADMIN_PUBKEY.
 * Auth state is stored in a cookie (noscha_auth_token) backed by R2 sessions.
 */

const AUTH_LOGIN_HTML = `<!DOCTYPE html>
<html lang="en">
<head>
<meta charset="UTF-8">
<meta name="viewport" content="width=device-width,initial-scale=1.0">
<title>noscha.io — Login Required</title>
<style>
*{margin:0;padding:0;box-sizing:border-box}
body{font-family:-apple-system,BlinkMacSystemFont,'Segoe UI',sans-serif;background:#0d0d0d;color:#e0e0e0;min-height:100vh;display:flex;align-items:center;justify-content:center}
.wrap{text-align:center;max-width:400px;padding:2rem}
.logo{font-size:2rem;font-weight:700;margin-bottom:1.5rem}
.logo span:first-child{color:#8b5cf6}
.logo span:nth-child(2){color:#f97316}
p{color:#888;margin-bottom:2rem;line-height:1.6}
button{background:#8b5cf6;color:#fff;border:none;padding:.8rem 2rem;border-radius:8px;font-size:1rem;font-weight:600;cursor:pointer}
button:hover{opacity:.9}
button:disabled{opacity:.5;cursor:not-allowed}
#error{color:#ef4444;margin-top:1rem;font-size:.85rem}
#status{color:#888;margin-top:1rem;font-size:.85rem}
</style>
</head>
<body>
<div class="wrap">
<div class="logo"><span>noscha</span><span>.io</span></div>
<p>This environment requires authentication.<br>Please sign in with your Nostr identity (NIP-07).</p>
<button id="login-btn" onclick="doLogin()">Login with Nostr</button>
<div id="status"></div>
<div id="error"></div>
</div>
<script>
async function doLogin(){
  const btn=document.getElementById('login-btn');
  const status=document.getElementById('status');
  const error=document.getElementById('error');
  error.textContent='';
  if(!window.nostr){error.textContent='No NIP-07 extension found. Please install nos2x, Alby, or similar.';return;}
  btn.disabled=true;status.textContent='Requesting challenge...';
  try{
    const cr=await fetch('/api/admin/challenge',{method:'POST'});
    if(!cr.ok)throw new Error('Failed to get challenge');
    const cd=await cr.json();
    status.textContent='Please sign the event...';
    const event=await window.nostr.signEvent({kind:27235,created_at:Math.floor(Date.now()/1000),tags:[],content:cd.challenge});
    status.textContent='Verifying...';
    const lr=await fetch('/api/auth/verify',{method:'POST',headers:{'Content-Type':'application/json'},body:JSON.stringify({event:JSON.stringify(event)})});
    if(!lr.ok){const t=await lr.text();throw new Error(t);}
    const ld=await lr.json();
    document.cookie='noscha_auth_token='+ld.token+';path=/;max-age=86400;SameSite=Lax';
    status.textContent='Authenticated! Redirecting...';
    setTimeout(()=>location.reload(),500);
  }catch(e){error.textContent=e.message;btn.disabled=false;status.textContent='';}
}
</script>
</body>
</html>`;

/**
 * Handle POST /api/auth/verify — verify NIP-07 signed event for staging auth
 */
async function handleAuthVerify(request, env) {
  try {
    const body = await request.json();
    const event = JSON.parse(body.event);

    // Check pubkey matches ADMIN_PUBKEY
    let adminPubkey;
    try {
      adminPubkey = env.ADMIN_PUBKEY;
    } catch {
      return new Response('ADMIN_PUBKEY not configured', { status: 500 });
    }
    if (!adminPubkey) {
      return new Response('ADMIN_PUBKEY not configured', { status: 500 });
    }

    if (event.pubkey !== adminPubkey) {
      return new Response('Unauthorized: pubkey not allowed', { status: 403 });
    }

    // Verify challenge exists in R2
    const challenge = (event.content || '').trim();
    if (!challenge) {
      return new Response('Missing challenge', { status: 400 });
    }

    const bucket = env.BUCKET;
    const chKey = `challenges/${challenge}.json`;
    const chObj = await bucket.get(chKey);
    if (!chObj) {
      return new Response('Invalid or expired challenge', { status: 400 });
    }
    // Delete used challenge
    await bucket.delete(chKey);

    // Create session token
    const token = 'staging_' + Date.now().toString(36) + '_' + Math.random().toString(36).slice(2);
    const expiresAt = new Date(Date.now() + 24 * 60 * 60 * 1000).toISOString();
    const session = { pubkey: event.pubkey, created_at: new Date().toISOString(), expires_at: expiresAt };
    await bucket.put(`staging_sessions/${token}.json`, JSON.stringify(session));

    return new Response(JSON.stringify({ token }), {
      headers: { 'Content-Type': 'application/json' },
    });
  } catch (e) {
    return new Response('Invalid request: ' + e.message, { status: 400 });
  }
}

/**
 * Check if request has valid staging auth cookie
 */
async function isAuthenticated(request, env) {
  const cookie = request.headers.get('Cookie') || '';
  const match = cookie.match(/noscha_auth_token=([^;]+)/);
  if (!match) return false;

  const token = match[1];
  const bucket = env.BUCKET;
  const obj = await bucket.get(`staging_sessions/${token}.json`);
  if (!obj) return false;

  try {
    const session = JSON.parse(await obj.text());
    const expiresMs = new Date(session.expires_at).getTime();
    if (Date.now() > expiresMs) {
      await bucket.delete(`staging_sessions/${token}.json`);
      return false;
    }
    return true;
  } catch {
    return false;
  }
}

/**
 * Find rental by management token by scanning R2
 */
async function findRentalByManagementToken(bucket, managementToken) {
  const list = await bucket.list({ prefix: 'rentals/' });

  for (const obj of list.objects) {
    try {
      const rental = JSON.parse(await bucket.get(obj.key).then(r => r?.text()));
      if (rental && rental.management_token === managementToken) {
        return { rental, key: obj.key };
      }
    } catch (e) {
      console.error(`Error reading rental ${obj.key}: ${e.message}`);
    }
  }

  return null;
}

/**
 * Handle GET /api/mail/{token} - Get email from inbox and mark as read
 */
async function handleGetMail(request, env, token) {
  try {
    const key = `inbox/${token}.json`;
    const obj = await env.BUCKET.get(key);

    if (!obj) {
      return new Response(JSON.stringify({ error: "Email not found" }), {
        status: 404,
        headers: { 'Content-Type': 'application/json' }
      });
    }

    const emailData = JSON.parse(await obj.text());

    // Mark as read on first access
    if (!emailData.read_at) {
      emailData.read_at = new Date().toISOString();
      await env.BUCKET.put(key, JSON.stringify(emailData));
    }

    return new Response(JSON.stringify(emailData), {
      headers: { 'Content-Type': 'application/json' }
    });
  } catch (e) {
    return new Response(JSON.stringify({ error: "Failed to retrieve email" }), {
      status: 500,
      headers: { 'Content-Type': 'application/json' }
    });
  }
}

/**
 * Handle POST /api/mail/send/{management_token} - Send email via Resend with rate limits
 */
async function handleSendMail(request, env, managementToken) {
  try {
    const rentalResult = await findRentalByManagementToken(env.BUCKET, managementToken);
    if (!rentalResult) {
      return new Response(JSON.stringify({ error: "Invalid management token" }), {
        status: 401,
        headers: { 'Content-Type': 'application/json' }
      });
    }

    const { rental, key } = rentalResult;
    const username = key.split('/')[1].replace('.json', '');

    // Check if rental is active and not expired
    if (rental.status !== "active") {
      return new Response(JSON.stringify({ error: "Address not active" }), {
        status: 403,
        headers: { 'Content-Type': 'application/json' }
      });
    }

    const now = new Date();
    const expires = new Date(rental.expires_at);
    if (now > expires) {
      return new Response(JSON.stringify({ error: "Address expired" }), {
        status: 403,
        headers: { 'Content-Type': 'application/json' }
      });
    }

    // Check email service is enabled
    if (!rental.services?.email?.enabled) {
      return new Response(JSON.stringify({ error: "Email service not enabled" }), {
        status: 403,
        headers: { 'Content-Type': 'application/json' }
      });
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
          emailCount = { count: 0, window_start: now.toISOString() };
        }
      }
    } catch (e) {
      console.error(`Rate limit check error: ${e.message}`);
    }

    if (emailCount.count >= DAILY_LIMIT) {
      return new Response(JSON.stringify({ error: "Daily email limit reached (5/day)" }), {
        status: 429,
        headers: { 'Content-Type': 'application/json' }
      });
    }

    // Parse request body
    const body = await request.json();
    const { to, subject, body: emailBody } = body;

    if (!to || !subject || !emailBody) {
      return new Response(JSON.stringify({ error: "Missing required fields: to, subject, body" }), {
        status: 400,
        headers: { 'Content-Type': 'application/json' }
      });
    }

    // Send via Resend
    const apiKey = env.RESEND_API_KEY;
    if (!apiKey) {
      return new Response(JSON.stringify({ error: "Email service temporarily unavailable" }), {
        status: 503,
        headers: { 'Content-Type': 'application/json' }
      });
    }

    const fromAddr = `${username}@noscha.io`;

    const res = await fetch("https://api.resend.com/emails", {
      method: "POST",
      headers: {
        "Authorization": `Bearer ${apiKey}`,
        "Content-Type": "application/json",
      },
      body: JSON.stringify({
        from: fromAddr,
        to: [to],
        subject: subject,
        text: emailBody,
      }),
    });

    if (!res.ok) {
      const errText = await res.text();
      console.error(`Resend API error: ${res.status} ${errText}`);
      return new Response(JSON.stringify({ error: "Failed to send email" }), {
        status: 500,
        headers: { 'Content-Type': 'application/json' }
      });
    }

    // Increment email count after successful send
    emailCount.count += 1;
    try {
      await env.BUCKET.put(countKey, JSON.stringify(emailCount));
    } catch (e) {
      console.error(`Failed to update email count: ${e.message}`);
    }

    const result = await res.json();
    return new Response(JSON.stringify({ success: true, message_id: result.id }), {
      headers: { 'Content-Type': 'application/json' }
    });
  } catch (e) {
    return new Response(JSON.stringify({ error: "Invalid request: " + e.message }), {
      status: 400,
      headers: { 'Content-Type': 'application/json' }
    });
  }
}

// Create a subclass that adds the email handler and auth gate
class UnifiedWorker extends WorkerClass {
  async fetch(request) {
    const url = new URL(request.url);
    const requireAuth = this.env.REQUIRE_AUTH === 'true';

    // Handle new email API routes
    if (url.pathname.startsWith('/api/mail/')) {
      const pathParts = url.pathname.split('/');

      // GET /api/mail/{token}
      if (request.method === 'GET' && pathParts.length === 4 && pathParts[3]) {
        const token = pathParts[3];
        return handleGetMail(request, this.env, token);
      }

      // POST /api/mail/send/{management_token}
      if (request.method === 'POST' && pathParts.length === 5 && pathParts[3] === 'send' && pathParts[4]) {
        const managementToken = pathParts[4];
        return handleSendMail(request, this.env, managementToken);
      }
    }

    if (requireAuth) {
      // POST /api/auth/verify — always allowed (auth endpoint)
      if (url.pathname === '/api/auth/verify' && request.method === 'POST') {
        return handleAuthVerify(request, this.env);
      }

      // /health — always allowed
      if (url.pathname === '/health') {
        return super.fetch(request);
      }

      // API endpoints — allow through (they have their own auth)
      // But protect HTML pages and root
      const isApi = url.pathname.startsWith('/api/');
      const isWellKnown = url.pathname.startsWith('/.well-known/');
      const isAgentDoc = url.pathname === '/skill.md' || url.pathname === '/llms.txt';

      if (!isApi && !isWellKnown && !isAgentDoc) {
        const authed = await isAuthenticated(request, this.env);
        if (!authed) {
          return new Response(AUTH_LOGIN_HTML, {
            headers: { 'Content-Type': 'text/html;charset=UTF-8' },
          });
        }
      }
    }

    return super.fetch(request);
  }

  async email(message) {
    return emailShim.email(message, this.env, this.ctx);
  }

  async scheduled(controller) {
    // Cleanup old emails from inbox
    try {
      const list = await this.env.BUCKET.list({ prefix: 'inbox/' });
      const now = new Date();

      for (const obj of list.objects) {
        try {
          const email = JSON.parse(await this.env.BUCKET.get(obj.key).then(r => r?.text()));
          if (!email) continue;

          const createdAt = new Date(email.created_at);
          const readAt = email.read_at ? new Date(email.read_at) : null;

          let shouldDelete = false;

          // Delete if read and 1 hour has passed since read
          if (readAt && (now - readAt) >= 60 * 60 * 1000) {
            shouldDelete = true;
          }

          // Delete if unread and 24 hours have passed since created
          if (!readAt && (now - createdAt) >= 24 * 60 * 60 * 1000) {
            shouldDelete = true;
          }

          if (shouldDelete) {
            await this.env.BUCKET.delete(obj.key);
            console.log(`Deleted old email: ${obj.key}`);
          }
        } catch (e) {
          console.error(`Error processing email ${obj.key}: ${e.message}`);
        }
      }
    } catch (e) {
      console.error(`Email cleanup error: ${e.message}`);
    }

    // Call parent scheduled method if it exists
    if (super.scheduled) {
      await super.scheduled(controller);
    }
  }
}

export default UnifiedWorker;
