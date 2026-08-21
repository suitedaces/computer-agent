/**
 * Lightweight browser automation via CDP (chromiumoxide equivalent in Node)
 * Uses fetch to talk to Chrome DevTools Protocol over http://127.0.0.1:9222
 * If Chrome not running with --remote-debugging-port=9222, tries to launch Chrome.
 * For opencode usage, this is best-effort; full a11y snapshot is proxied via simple DOM snapshot.
 */
export declare function browserSnapshot(verbose?: boolean): Promise<string>;
export declare function browserNavigate(url?: string, action?: string): Promise<string>;
export declare function browserTabs(): Promise<string>;
export declare function browserClick(uid: string): Promise<string>;
export declare function browserType(uid: string, text: string): Promise<string>;
