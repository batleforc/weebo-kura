import type { CreateClientConfig } from "./generated/client.gen";

/** Same origin as the API (RFC 0003 §6.1); axios has no timeout by default. */
export const createClientConfig: CreateClientConfig = (c) => ({
  ...c,
  // The generated paths already start with /v1.
  baseURL: "",
  timeout: 30_000,
});
