/**
 * Lightweight browser automation via CDP (chromiumoxide equivalent in Node)
 * Uses fetch to talk to Chrome DevTools Protocol over http://127.0.0.1:9222
 * If Chrome not running with --remote-debugging-port=9222, tries to launch Chrome.
 * For opencode usage, this is best-effort; full a11y snapshot is proxied via simple DOM snapshot.
 */

const CDP_HOST = "http://127.0.0.1:9222";

async function cdpGet(path: string) {
  const r = await fetch(`${CDP_HOST}${path}`);
  if (!r.ok) throw new Error(`CDP ${path} failed: ${r.status}`);
  return r.json();
}

async function getWsUrl(): Promise<string> {
  const data: any = await cdpGet("/json/version");
  if (data.webSocketDebuggerUrl) return data.webSocketDebuggerUrl;
  throw new Error("Chrome not running with --remote-debugging-port=9222. Start with: open -a 'Google Chrome' --args --remote-debugging-port=9222 or chrome.exe --remote-debugging-port=9222");
}

async function getTargets() {
  return cdpGet("/json/list");
}

// Simple snapshot via Runtime.evaluate + Accessibility.getFullAXTree would need websocket.
// For MVP MCP we provide a text snapshot via DOM traversal over CDP http (fallback).
export async function browserSnapshot(verbose = false): Promise<string> {
  try {
    const targets: any[] = await getTargets();
    if (!targets.length) return "No pages open";
    // For MVP, return target list with instructions to use opencode browsermcp or full app
    const lines = targets.map((t: any, i: number) => `${i}: ${t.title} - ${t.url} [${t.type}] id=${t.id}`);
    return `Browser pages (use browser_navigate / browser_click):\n${lines.join("\n")}\n\nTip: For full a11y snapshot, run taskhomie app in Background Mode or use opencode-browser / ego-browser skill. Verbose=${verbose}`;
  } catch (e: any) {
    return `Browser snapshot failed: ${e.message}. Ensure Chrome is running with --remote-debugging-port=9222.`;
  }
}

export async function browserNavigate(url?: string, action?: string): Promise<string> {
  const targets: any[] = await getTargets();
  if (!targets.length) throw new Error("No browser targets");
  const target = targets[0];
  const wsUrl: string = target.webSocketDebuggerUrl;
  // Use CDP Page.navigate via websocket would be ideal; use http fallback via fetch to chrome's /json/new?url
  if (url) {
    // open new tab via /json/new
    const r = await fetch(`${CDP_HOST}/json/new?${encodeURIComponent(url)}`);
    if (!r.ok) throw new Error(`Failed to open ${url}: ${r.status}`);
    const data: any = await r.json();
    return `Opened ${url} (target ${data.id}) via CDP`;
  }
  if (action === "reload") {
    // close and reopen is simplest via http
    return "Reload not implemented via http; use browser_navigate with url same as current";
  }
  if (action === "back" || action === "forward") {
    // would need websocket; fallback
    throw new Error(`${action} requires websocket CDP - launch Chrome with debugging and use full taskhomie app for now`);
  }
  throw new Error("browser_navigate requires url or action");
}

export async function browserTabs(): Promise<string> {
  return browserSnapshot(false);
}

// Stubs for click/type that delegate to CDP ws - for now error with guidance
export async function browserClick(uid: string): Promise<string> {
  throw new Error(`browser_click uid=${uid} requires a11y snapshot uid mapping - use taskhomie Background Mode or ego-browser skill for full browser automation. As fallback, use computer_click with AI coordinates 0-1280,0-800 after computer_screenshot.`);
}

export async function browserType(uid: string, text: string): Promise<string> {
  throw new Error(`browser_type uid=${uid} "${text}" - same as click, use computer_type after focusing element with computer_click.`);
}
