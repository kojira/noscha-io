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

// Create a subclass that adds the email handler and auth gate
class UnifiedWorker extends WorkerClass {
  async fetch(request) {
    const url = new URL(request.url);
    const requireAuth = this.env.REQUIRE_AUTH === 'true';

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

      if (!isApi && !isWellKnown) {
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
}

export default UnifiedWorker;
