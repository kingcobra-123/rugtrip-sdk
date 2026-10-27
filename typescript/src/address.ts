const HEX_RUN = /^0[xX][0-9a-fA-F]+$/;

function exact(s: string): string | undefined {
  if (!HEX_RUN.test(s)) return undefined;
  const n = s.length - 2;
  return n === 40 || n === 64 ? s : undefined;
}

/**
 * Turn a token address, pool address, v4 pool id or chart link into the path segment the API expects.
 * Links are reduced to the last 40- or 64-hex segment of their path. Anything else is sent URL-encoded and the
 * server answers `bad_address`. Case is kept, so the server can check EIP-55 checksums.
 */
export function toTarget(input: string): string {
  const s = input.trim();
  const direct = exact(s);
  if (direct) return direct;
  if (s.includes("/")) {
    const path = s.split(/[?#]/)[0] ?? "";
    const segs = path.split("/");
    for (let i = segs.length - 1; i >= 0; i--) {
      const seg = segs[i];
      let decoded = seg;
      try {
        decoded = decodeURIComponent(seg);
      } catch {
        /* keep raw */
      }
      const hit = exact(decoded);
      if (hit) return hit;
    }
  }
  return encodeURIComponent(s);
}
