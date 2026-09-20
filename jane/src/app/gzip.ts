// Text <-> gzip bytes through the platform's CompressionStream. No DOM, no storage:
// the same code runs in the browser and under Node (18+), which is what lets the
// test suite prove the round trip on a real save. A save is JSON, which is mostly
// repeated key names, so gzip takes it to a small fraction of its size for free.

/** Older browsers have no CompressionStream. The caller stores plain text instead. */
export function canGzip(): boolean {
  return typeof CompressionStream === "function" && typeof DecompressionStream === "function";
}

export async function gzipText(text: string): Promise<Uint8Array> {
  return pump(new Blob([text]).stream(), new CompressionStream("gzip"));
}

/** Rejects on bytes that are not gzip. The caller reads that as "slot is empty". */
export async function gunzipText(bytes: Uint8Array): Promise<string> {
  const out = await pump(new Blob([bytes as BlobPart]).stream(), new DecompressionStream("gzip"));
  return new TextDecoder().decode(out);
}

async function pump(source: ReadableStream<Uint8Array>, through: CompressionStream | DecompressionStream): Promise<Uint8Array> {
  // Response is the shortest correct way to drain a stream into one buffer.
  return new Uint8Array(await new Response(source.pipeThrough(through as ReadableWritablePair<Uint8Array, Uint8Array>)).arrayBuffer());
}
