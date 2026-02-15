import contextlib
import socket
import threading
import time
import unittest

import zpl_toolchain


class MockPrinterServer:
    def __init__(self) -> None:
        self._sock = socket.socket(socket.AF_INET, socket.SOCK_STREAM)
        self._sock.bind(("127.0.0.1", 0))
        self._sock.listen(5)
        self._sock.settimeout(0.2)
        self.host = "127.0.0.1"
        self.port = self._sock.getsockname()[1]
        self.received_payloads: list[str] = []
        self._running = True
        self._thread = threading.Thread(target=self._serve, daemon=True)
        self._thread.start()

    def _serve(self) -> None:
        while self._running:
            try:
                conn, _ = self._sock.accept()
            except socket.timeout:
                continue
            except OSError:
                break

            with conn:
                conn.settimeout(0.2)
                received = bytearray()
                try:
                    while True:
                        chunk = conn.recv(4096)
                        if not chunk:
                            break
                        received.extend(chunk)
                except socket.timeout:
                    pass
                except OSError:
                    continue
                payload = bytes(received)
                if not payload:
                    continue
                text = payload.decode("utf-8", errors="replace")
                self.received_payloads.append(text)

    def close(self) -> None:
        self._running = False
        with contextlib.suppress(OSError):
            self._sock.close()
        self._thread.join(timeout=1)


class PythonBindingApiTests(unittest.TestCase):
    def test_parse_returns_ast_dict(self) -> None:
        result = zpl_toolchain.parse("^XA^FO50,50^FDHELLO^FS^XZ")
        self.assertIsInstance(result, dict)
        self.assertIn("ast", result)
        self.assertGreater(len(result["ast"]["labels"]), 0)

    def test_format_returns_string(self) -> None:
        formatted = zpl_toolchain.format("^XA^FD Hello ^FS^XZ", "label")
        self.assertIsInstance(formatted, str)
        self.assertIn("^XA", formatted)

    def test_format_with_compaction_compacts_field_block_with_label_indent(self) -> None:
        input_zpl = "^XA\n^PW609\n^LL406\n^FO30,30\n^A0N,35,35\n^FDWIDGET-3000\n^FS\n^XZ\n"
        formatted = zpl_toolchain.format(input_zpl, "label", "field")
        self.assertIn("  ^FO30,30^A0N,35,35^FDWIDGET-3000^FS", formatted)

    def test_format_comment_placement_line_keeps_comment_on_new_line(self) -> None:
        input_zpl = "^XA\n^PW812\n; set print width\n^XZ\n"
        formatted = zpl_toolchain.format(input_zpl, "none", "none", "line")
        self.assertIn("^PW812\n; set print width", formatted)

    def test_explain_unknown_returns_none(self) -> None:
        self.assertIsNone(zpl_toolchain.explain("ZPL9999"))

    def test_parse_with_tables_invalid_json_raises(self) -> None:
        with self.assertRaises(ValueError):
            zpl_toolchain.parse_with_tables("^XA^XZ", "{invalid")

    def test_validate_with_tables_invalid_json_raises(self) -> None:
        with self.assertRaises(ValueError):
            zpl_toolchain.validate_with_tables("^XA^XZ", "{invalid")

    def test_print_with_options_rejects_zero_timeout(self) -> None:
        with self.assertRaises(RuntimeError) as ctx:
            zpl_toolchain.print_zpl_with_options(
                "^XA^XZ",
                "127.0.0.1:9100",
                None,
                False,
                0,
                None,
            )
        self.assertIn("timeout_ms must be > 0", str(ctx.exception))

    def test_query_status_with_options_rejects_zero_timeout(self) -> None:
        with self.assertRaises(RuntimeError) as ctx:
            zpl_toolchain.query_printer_status_with_options("127.0.0.1:9100", 0, None)
        self.assertIn("timeout_ms must be > 0", str(ctx.exception))

    def test_query_info_with_options_rejects_zero_timeout(self) -> None:
        with self.assertRaises(RuntimeError) as ctx:
            zpl_toolchain.query_printer_info_with_options("127.0.0.1:9100", 0, None)
        self.assertIn("timeout_ms must be > 0", str(ctx.exception))

    def test_print_zpl_sends_payload_to_mock_printer(self) -> None:
        server = MockPrinterServer()
        try:
            result = zpl_toolchain.print_zpl("^XA^FO20,20^FDTEST^FS^XZ", f"{server.host}:{server.port}", None, False)
            for _ in range(20):
                if server.received_payloads:
                    break
                time.sleep(0.01)
        finally:
            server.close()
        self.assertTrue(result["success"])
        self.assertGreater(result["bytes_sent"], 0)
        self.assertTrue(any("^XA" in payload for payload in server.received_payloads))

if __name__ == "__main__":
    unittest.main()
