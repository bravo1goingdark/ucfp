// Cloudflare Turnstile server-side verification.
// https://developers.cloudflare.com/turnstile/get-started/server-side-validation/

const VERIFY_URL = 'https://challenges.cloudflare.com/turnstile/v0/siteverify';

export interface TurnstileResult {
  success: boolean;
  errorCodes?: string[];
}

export async function verifyTurnstile(
  secret: string | undefined,
  token: string | null,
  remoteIp?: string | null
): Promise<TurnstileResult> {
  // If Turnstile isn't configured, we treat as trivially-passing in dev.
  // Production deploys MUST set TURNSTILE_SECRET; the auth/demo handlers
  // refuse to skip verification when they expect it.
  if (!secret) return { success: true };
  if (!token) return { success: false, errorCodes: ['missing-input-response'] };

  const body = new URLSearchParams();
  body.set('secret', secret);
  body.set('response', token);
  if (remoteIp) body.set('remoteip', remoteIp);

  try {
    const res = await fetch(VERIFY_URL, {
      method: 'POST',
      headers: { 'content-type': 'application/x-www-form-urlencoded' },
      body
    });
    const json = (await res.json()) as { success: boolean; 'error-codes'?: string[] };
    return { success: json.success, errorCodes: json['error-codes'] };
  } catch {
    return { success: false, errorCodes: ['network-error'] };
  }
}
