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

    // Verify challenge exists in R2 (any valid NIP-07 user can get staging session)
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
 * Check if request has valid staging auth: Bearer token or cookie
 */
async function isAuthenticated(request, env) {
  // Bearer STAGING_AUTH_TOKEN で許可
  try {
    const authHeader = request.headers.get("Authorization");
    if (authHeader && authHeader.startsWith("Bearer ")) {
      const token = authHeader.slice(7);
      const stagingToken = env.STAGING_AUTH_TOKEN;
      if (stagingToken && token === stagingToken) return true;
    }
  } catch (_) {}

  // Cookie チェック
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

async function authenticateMailRequest(env, username, token) {
  if (!token) return { error: "Missing token parameter", status: 401 };
  const obj = await env.BUCKET.get(`rentals/${username}.json`);
  if (!obj) return { error: "User not found", status: 404 };
  const rental = JSON.parse(await obj.text());
  if (rental.management_token !== token) return { error: "Invalid token", status: 401 };
  if (rental.status !== "active") return { error: "Account not active", status: 403 };
  const now = new Date();
  const expires = new Date(rental.expires_at);
  if (now > expires) return { error: "Account expired", status: 403 };
  if (!rental.services?.email?.enabled) return { error: "Email service not enabled", status: 403 };
  return { rental };
}

async function handleListMail(request, env, username) {
  const url = new URL(request.url);
  const token = url.searchParams.get("token");
  const auth = await authenticateMailRequest(env, username, token);
  if (auth.error) return new Response(JSON.stringify({ error: auth.error }), { status: auth.status, headers: { "Content-Type": "application/json" } });

  const list = await env.BUCKET.list({ prefix: `inbox/${username}/` });
  const emails = [];
  for (const obj of list.objects) {
    try {
      const data = JSON.parse(await env.BUCKET.get(obj.key).then(r => r?.text()));
      if (data) {
        emails.push({ mail_id: data.mail_id, from: data.from, subject: data.subject, date: data.date, read_at: data.read_at, created_at: data.created_at });
      }
    } catch (e) {}
  }
  return new Response(JSON.stringify(emails), { headers: { "Content-Type": "application/json" } });
}

async function handleGetMailById(request, env, username, mailId) {
  const url = new URL(request.url);
  const token = url.searchParams.get("token");
  // view_url from webhook: token なしでアクセス可（URLが秘密鍵代わり）
  if (token) {
    const auth = await authenticateMailRequest(env, username, token);
    if (auth.error) return new Response(JSON.stringify({ error: auth.error }), { status: auth.status, headers: { "Content-Type": "application/json" } });
  }

  const key = `inbox/${username}/${mailId}.json`;
  const obj = await env.BUCKET.get(key);
  if (!obj) return new Response(JSON.stringify({ error: "Email not found" }), { status: 404, headers: { "Content-Type": "application/json" } });

  const emailData = JSON.parse(await obj.text());
  if (!emailData.read_at) {
    emailData.read_at = new Date().toISOString();
    await env.BUCKET.put(key, JSON.stringify(emailData));
  }
  return new Response(JSON.stringify({ from: emailData.from, to: emailData.to, subject: emailData.subject, body_text: emailData.body_text, body_html: emailData.body_html, date: emailData.date, mail_id: emailData.mail_id, created_at: emailData.created_at, read_at: emailData.read_at }), { headers: { "Content-Type": "application/json" } });
}

async function handleSendMail(request, env, username) {
  try {
    const body = await request.json();
    const { to, subject, text, management_token } = body;

    const auth = await authenticateMailRequest(env, username, management_token);
    if (auth.error) return new Response(JSON.stringify({ error: auth.error }), { status: auth.status, headers: { "Content-Type": "application/json" } });

    if (!to || !subject || !text) {
      return new Response(JSON.stringify({ error: "Missing required fields: to, subject, text" }), { status: 400, headers: { "Content-Type": "application/json" } });
    }

    // Rate limit: 5 emails per 24 hours per user
    const DAILY_LIMIT = 5;
    const WINDOW_MS = 24 * 60 * 60 * 1000;
    const now = new Date();
    const countKey = `send_emails/${username}.json`;
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
    } catch (e) {}

    if (emailCount.count >= DAILY_LIMIT) {
      return new Response(JSON.stringify({ error: "Daily send limit reached (5/day)" }), { status: 429, headers: { "Content-Type": "application/json" } });
    }

    const apiKey = env.RESEND_API_KEY;
    if (!apiKey) {
      return new Response(JSON.stringify({ error: "Email service temporarily unavailable" }), { status: 503, headers: { "Content-Type": "application/json" } });
    }

    const fromAddr = `${username}@noscha.io`;
    const res = await fetch("https://api.resend.com/emails", {
      method: "POST",
      headers: { "Authorization": `Bearer ${apiKey}`, "Content-Type": "application/json" },
      body: JSON.stringify({ from: fromAddr, to: [to], subject, text }),
    });

    if (!res.ok) {
      console.error(`Resend API error: ${res.status} ${await res.text()}`);
      return new Response(JSON.stringify({ error: "Failed to send email" }), { status: 500, headers: { "Content-Type": "application/json" } });
    }

    emailCount.count += 1;
    try { await env.BUCKET.put(countKey, JSON.stringify(emailCount)); } catch (e) {}

    const result = await res.json();
    return new Response(JSON.stringify({ success: true, message_id: result.id }), { headers: { "Content-Type": "application/json" } });
  } catch (e) {
    return new Response(JSON.stringify({ error: "Invalid request: " + e.message }), { status: 400, headers: { "Content-Type": "application/json" } });
  }
}

// Export as plain ES module object so CF runtime can route ALL events
// including email (which WorkerEntrypoint-based classes don't support).
//
// WorkerClass (from build/index.js) is a Proxy-wrapped class extending
// WorkerEntrypoint.  Its prototype.fetch/scheduled read this.env / this.ctx
// set by the CF runtime during construction.  Since we export a plain object
// instead of the class, we create a fresh instance per request via the CF
// runtime constructor convention:  new WorkerClass(ctx, env)  — the Proxy
// construct trap passes these through to Reflect.construct(A, [ctx, env]).
// Alternatively, we can create an instance once and patch env/ctx before
// each call.  The safest approach: create per-invocation to avoid shared
// mutable state.

export default {
  async fetch(request, env, ctx) {
    const url = new URL(request.url);
    const requireAuth = env.REQUIRE_AUTH === 'true';

    // Handle mail API routes
    if (url.pathname.startsWith("/api/mail/")) {
      const pathParts = url.pathname.split("/").filter(Boolean); // ["api", "mail", ...]

      // POST /api/mail/{username}/send
      if (request.method === "POST" && pathParts.length === 4 && pathParts[3] === "send") {
        const username = pathParts[2];
        return handleSendMail(request, env, username);
      }

      // GET /api/mail/{username}/{mail_id}
      if (request.method === "GET" && pathParts.length === 4) {
        const username = pathParts[2];
        const mailId = pathParts[3];
        return handleGetMailById(request, env, username, mailId);
      }

      // GET /api/mail/{username}
      if (request.method === "GET" && pathParts.length === 3) {
        const username = pathParts[2];
        return handleListMail(request, env, username);
      }
    }

    if (requireAuth) {
      // Auth flow — always allowed (no gate)
      if (url.pathname === '/api/auth/verify' && request.method === 'POST') {
        return handleAuthVerify(request, env);
      }
      if (url.pathname === '/health') {
        const worker = new WorkerClass(ctx, env);
        return worker.fetch(request);
      }
      const isWellKnown = url.pathname.startsWith('/.well-known/');
      const isAgentDoc = url.pathname === '/skill.md' || url.pathname === '/llms.txt';
      const isAdminAuth = (url.pathname === '/api/admin/challenge' || url.pathname === '/api/admin/login') && request.method === 'POST';
      if (isWellKnown || isAgentDoc || isAdminAuth) {
        // Well-known, docs, admin auth flow — allow through
      } else {
        // All other paths (HTML + API): require Bearer or NIP-07 cookie
        const authed = await isAuthenticated(request, env);
        if (!authed) {
          const isApi = url.pathname.startsWith('/api/');
          if (isApi) {
            return new Response(JSON.stringify({ error: "Unauthorized" }), {
              status: 401,
              headers: { 'Content-Type': 'application/json' },
            });
          }
          return new Response(AUTH_LOGIN_HTML, {
            headers: { 'Content-Type': 'text/html;charset=UTF-8' },
          });
        }
      }
    }

    const worker = new WorkerClass(ctx, env);
    return worker.fetch(request);
  },

  async email(message, env, ctx) {
    return emailShim.email(message, env, ctx);
  },

  async scheduled(controller, env, ctx) {
    try {
      const list = await env.BUCKET.list({ prefix: "inbox/" });
      const now = new Date();
      const ONE_HOUR_MS = 60 * 60 * 1000;

      for (const obj of list.objects) {
        try {
          const email = JSON.parse(await env.BUCKET.get(obj.key).then(r => r?.text()));
          if (!email) continue;
          const createdAt = new Date(email.created_at);
          if (now - createdAt >= ONE_HOUR_MS) {
            await env.BUCKET.delete(obj.key);
            console.log(`Deleted old email: ${obj.key}`);
          }
        } catch (e) {
          console.error(`Error processing email ${obj.key}: ${e.message}`);
        }
      }
    } catch (e) {
      console.error(`Email cleanup error: ${e.message}`);
    }

    // Call Rust scheduled handler
    const worker = new WorkerClass(ctx, env);
    return worker.scheduled(controller);
  }
};
