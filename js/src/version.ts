// This is free and unencumbered software released into the public domain.

import config from "../jsr.json" with { type: "json" };

export function version(): string {
  return config.version;
}
