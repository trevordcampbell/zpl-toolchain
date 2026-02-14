import net from "node:net";

/**
 * Detect whether this environment can bind localhost TCP sockets.
 * Some sandboxed runtimes deny bind/listen, which breaks integration-style
 * network tests that are otherwise valid in CI/dev environments.
 */
export async function canBindLocalTcp(): Promise<boolean> {
  const server = net.createServer();
  try {
    await new Promise<void>((resolve, reject) => {
      server.once("error", reject);
      server.listen(0, "127.0.0.1", () => {
        server.removeListener("error", reject);
        resolve();
      });
    });
    return true;
  } catch {
    return false;
  } finally {
    await new Promise<void>((resolve) => {
      server.close(() => resolve());
    }).catch(() => undefined);
  }
}

