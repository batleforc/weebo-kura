/**
 * The one script the note frame runs (RFC 0005 §6.5): it reports the
 * document's height and the link a user clicked to the application, and
 * nothing else. The server's Content-Security-Policy allows it by its hash,
 * written at build time to `dist/frame-csp.txt`; any other script, whatever a
 * note carries, is refused. Keep it byte for byte: the hash is of this text.
 */
export const BRIDGE =
  '(()=>{const post=(kind,value)=>parent.postMessage({kind,value},"*");' +
  'const size=()=>post("height",document.documentElement.scrollHeight);' +
  'new ResizeObserver(size).observe(document.body);addEventListener("load",size);' +
  'document.addEventListener("click",(e)=>{const a=e.target instanceof Element?e.target.closest("a[href]"):null;' +
  'if(!a)return;e.preventDefault();post("link",a.getAttribute("href"))})})();';

/** `'sha256-…'`, the CSP source of the bridge. */
export async function bridgeHash(text: string = BRIDGE): Promise<string> {
  const digest = await crypto.subtle.digest(
    "SHA-256",
    new TextEncoder().encode(text),
  );
  let binary = "";
  for (const byte of new Uint8Array(digest))
    binary += String.fromCharCode(byte);
  return `'sha256-${btoa(binary)}'`;
}
