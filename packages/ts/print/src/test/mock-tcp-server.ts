import net from "node:net";

export interface MockTcpServerOptions {
  failConnectAttempts?: number;
  failAfterPayloads?: number;
}

export interface MockTcpServerHandle {
  port: number;
  close: () => Promise<void>;
  receivedPayloads: string[];
}

const HS_RESPONSE =
  "\x020,0,0,1218,0,0,0,0,0,0,0,0\x03\r\n" +
  "\x020,0,0,0,0,0,0,0,0,0\x03\r\n" +
  "\x020,0\x03\r\n";

const HI_RESPONSE = '{"model":"ZD421","firmware":"V1.0","dpi":203,"memory_kb":65536}\r\n';

export async function createMockTcpServer(
  options: MockTcpServerOptions = {},
): Promise<MockTcpServerHandle> {
  let remainingFailedConnects = options.failConnectAttempts ?? 0;
  const receivedPayloads: string[] = [];
  const sockets = new Set<net.Socket>();
  const server = net.createServer((socket) => {
    sockets.add(socket);
    socket.on("close", () => {
      sockets.delete(socket);
    });

    if (remainingFailedConnects > 0) {
      remainingFailedConnects -= 1;
      socket.destroy();
      return;
    }
    socket.on("data", (chunk) => {
      const payload = chunk.toString("utf-8");
      receivedPayloads.push(payload);
      if (
        options.failAfterPayloads &&
        receivedPayloads.length >= options.failAfterPayloads
      ) {
        // Simulate a connection drop after N payloads, then stop accepting
        // new connections so callers exercise retry/failure paths.
        socket.destroy();
        server.close();
        return;
      }
      if (payload.includes("~HS")) {
        socket.write(HS_RESPONSE);
        return;
      }
      if (payload.includes("~HI")) {
        socket.write(HI_RESPONSE);
        return;
      }
    });
  });

  await new Promise<void>((resolve, reject) => {
    server.once("error", reject);
    server.listen(0, "127.0.0.1", () => {
      server.removeListener("error", reject);
      resolve();
    });
  });

  const address = server.address();
  if (!address || typeof address === "string") {
    throw new Error("Failed to determine mock TCP server port");
  }

  return {
    port: address.port,
    receivedPayloads,
    close: async () => {
      for (const socket of sockets) {
        socket.destroy();
      }
      await new Promise<void>((resolve, reject) => {
        server.close((err) => {
          if (!err) {
            resolve();
            return;
          }
          if ((err as NodeJS.ErrnoException).code === "ERR_SERVER_NOT_RUNNING") {
            resolve();
            return;
          }
          reject(err);
        });
      });
    },
  };
}
