// esbuild's `binary` loader: the WASM core as bytes inside main.js (§6.8).
declare module "*.wasm" {
  const bytes: Uint8Array;
  export default bytes;
}
