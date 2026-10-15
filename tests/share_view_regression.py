"""Preview contracts through real binaries; localhost fixtures, no cloud resources.

Run: python3 tests/share_view_regression.py [unified-binary] [standalone-binary]
"""
import http.server
import json
import os
from pathlib import Path
import pty
import subprocess
import sys
import tempfile
import threading
from urllib.parse import parse_qs, urlsplit


class PreviewScenario(http.server.BaseHTTPRequestHandler):
    mode = "document"
    requests = []
    payload = b'ordinary JSON {"PreviewURL":"not-a-descriptor"}\n'

    def log_message(self, *_arguments):
        pass

    def respond(self, payload, status=200, headers=None):
        encoded = json.dumps(payload).encode() if isinstance(payload, dict) else payload
        self.send_response(status)
        self.send_header("Content-Length", str(len(encoded)))
        self.send_header("x-ids-request-id", "preview-fixture")
        for name, value in (headers or {}).items():
            self.send_header(name, value)
        self.end_headers()
        try:
            self.wfile.write(encoded)
        except (BrokenPipeError, ConnectionResetError):
            pass

    def do_POST(self):
        self.rfile.read(int(self.headers.get("Content-Length", "0")))
        self.requests.append(self.path)
        self.respond({"AccessToken": "fixture-share-secret", "ExpiresIn": 60})

    def do_GET(self):
        self.requests.append(self.path)
        parsed = urlsplit(self.path)
        query = parse_qs(parsed.query)
        assert not self.headers.get("Authorization"), "preview leaked account auth"
        if parsed.path == "/v1/share_link/get_by_anonymous":
            self.respond({"ShareID": "s", "Permission": "view"})
        elif parsed.path == "/v1/share_file/list":
            assert query["list_root"] == ["true"]
            folders = [{"Folder": "docs/"}] if self.mode == "folder" else []
            files = [] if folders else [{"FilePath": "a.txt", "FileType": "file"}]
            self.respond({"Files": files, "Folders": folders, "IsTruncated": False})
        elif parsed.path == "/v1/share_file/get":
            selected = "other.txt" if self.mode == "wrong-file" else query["file_path"][0]
            self.respond({"FilePath": selected, "FileType": "file"})
        elif parsed.path == "/v1/share_file/preview":
            assert self.headers.get("x-share-token") == "fixture-share-secret"
            self.preview(query)
        else:
            raise AssertionError(f"unexpected preview route {parsed.path}")

    def preview(self, query):
        if self.mode.endswith(("-large", "-truncated")):
            self.send_response(403 if self.mode.startswith("denied") else 404)
            self.send_header("Content-Length", str(9 * 1024 * 1024 if self.mode.endswith("-large") else 128))
            self.send_header("x-ids-request-id", "refusal-fixture")
            self.end_headers()
            self.wfile.write(b"{")
        elif self.mode == "truncated":
            self.send_response(200)
            self.send_header("Content-Length", "128")
            self.end_headers()
            self.wfile.write(b"prefix")
        elif self.mode in ("denied", "missing", "server"):
            status = {"denied": 403, "missing": 404, "server": 500}[self.mode]
            self.respond({"Code": "Forbidden.SharePreviewDisabled" if status == 403 else "PreviewUnavailable"}, status)
        elif self.mode in ("document", "bad-url", "malformed"):
            assert query["preview_mode"] == ["auto"]
            address = "javascript:secret" if self.mode == "bad-url" else "https://preview.example.test/session?key=capability"
            descriptor = {"PreviewURL": address, "ExpiresAt": "2099-10-15T12:00:00Z", "PreviewType": "pdf"}
            self.respond(b"{" if self.mode == "malformed" else descriptor,
                         headers={"X-Ids-Preview-Result": " Document "})
        else:
            assert ("preview_mode" in query) == (self.mode != "raw")
            self.respond(self.payload, headers={"Content-Type": "application/json", "ETag": '"preview-tag"'})


def invoke(binary, root, origin, arguments=(), terminal=False):
    prefix = [] if binary.stem == "ve-adrive-cli" else ["ve-adrive"]
    environment = {key: value for key, value in os.environ.items()
                   if not key.startswith(("ADRIVE_", "TOS_OUTPUT"))}
    argv = [str(binary), *prefix, "share", "view", f"{origin}/share/s?pwd=fixture-password",
            *arguments, "--config-path", str(root / "missing-config"),
            "--credentials-path", str(root / "missing-credentials")]
    if not terminal:
        return subprocess.run(argv, capture_output=True, env=environment, timeout=20)
    master, slave = pty.openpty()
    try:
        return subprocess.run(argv, stdout=slave, stderr=subprocess.PIPE, env=environment, timeout=20)
    finally:
        os.close(master)
        os.close(slave)


def check_descriptor_and_raw(binary, root, origin):
    PreviewScenario.mode = "document"
    result = invoke(binary, root, origin, ["--path", "docs/a.docx", "--output", "json"])
    assert result.returncode == 0, result.stdout + result.stderr
    payload = json.loads(result.stdout)["data"]
    assert payload["ResultType"] == "document" and payload["PreviewType"] == "pdf"
    assert payload["PreviewURL"].startswith("https://preview.example.test/")
    PreviewScenario.mode = "metadata"
    result = invoke(binary, root, origin, ["--output", "json"])
    assert result.returncode == 0, result.stdout + result.stderr
    payload = json.loads(result.stdout)["data"]
    assert payload["ResultType"] == "raw" and payload["FilePath"] == "a.txt"
    assert "PreviewURL" not in payload
    PreviewScenario.mode = "raw"
    result = invoke(binary, root, origin, ["--path", "a.txt", "--raw"])
    assert result.returncode == 0 and result.stdout == PreviewScenario.payload


def check_failures(binary, root, origin):
    for mode in ("denied", "missing", "server", "bad-url", "malformed",
                 "denied-large", "missing-large", "denied-truncated", "missing-truncated"):
        PreviewScenario.mode = mode
        start = len(PreviewScenario.requests)
        result = invoke(binary, root, origin, ["--path", "a.txt", "--output", "json"])
        assert result.returncode != 0, mode
        requests = PreviewScenario.requests[start:]
        assert sum("/preview?" in path for path in requests) == 1, requests
        if mode.startswith(("denied", "missing", "server")):
            status = {"denied": 403, "missing": 404, "server": 500}[mode.split("-")[0]]
            envelope = json.loads(result.stdout)
            assert envelope["status_code"] == status
            if "-" in mode:
                assert envelope["request_id"] == "refusal-fixture"
                assert result.returncode == (5 if status == 403 else 4)
        if mode in ("bad-url", "malformed"):
            assert json.loads(result.stdout)["request_id"] == "preview-fixture"
        assert b"fixture-share-secret" not in result.stdout + result.stderr
        assert b"fixture-password" not in result.stdout + result.stderr
    PreviewScenario.mode = "folder"
    start = len(PreviewScenario.requests)
    assert invoke(binary, root, origin).returncode != 0
    assert not any("/preview?" in path for path in PreviewScenario.requests[start:])


def check_preflight(binary, root, origin):
    for arguments in (["--raw", "--output", "json"], ["--raw", "--query", "data"], ["--path", "docs/"]):
        start = len(PreviewScenario.requests)
        result = invoke(binary, root, origin, arguments)
        assert result.returncode != 0 and not result.stdout
        assert len(PreviewScenario.requests) == start
    PreviewScenario.mode = "wrong-file"
    start = len(PreviewScenario.requests)
    result = invoke(binary, root, origin, ["--raw", "--path", "requested.txt"])
    assert result.returncode != 0 and not result.stdout
    assert not any("/preview?" in path for path in PreviewScenario.requests[start:])
    start = len(PreviewScenario.requests)
    assert invoke(binary, root, origin, ["--raw"], terminal=True).returncode != 0
    assert len(PreviewScenario.requests) == start
    for arguments in (["--describe"], ["--dry-run", "--raw"]):
        start = len(PreviewScenario.requests)
        assert invoke(binary, root, origin, arguments).returncode == 0
        assert len(PreviewScenario.requests) == start


def check_stream_limits(binary, root, origin):
    PreviewScenario.mode = "raw"
    original = PreviewScenario.payload
    try:
        PreviewScenario.payload = b"x" * (9 * 1024 * 1024) + b"\x00\xff\x1b"
        result = invoke(binary, root, origin, ["--raw", "--path", "large.bin"])
        assert result.returncode == 0 and result.stdout == PreviewScenario.payload
    finally:
        PreviewScenario.payload = original
    PreviewScenario.mode = "truncated"
    start = len(PreviewScenario.requests)
    result = invoke(binary, root, origin, ["--raw", "--path", "a.txt"])
    assert result.returncode != 0 and result.stdout in (b"", b"prefix")
    assert b"not retried" in result.stderr
    assert sum("/preview?" in path for path in PreviewScenario.requests[start:]) == 1


def main():
    binaries = [Path(value).resolve() for value in sys.argv[1:]]
    if not binaries:
        binaries = [Path("target/debug/ve-storage-uni-cli").resolve()]
    server = http.server.ThreadingHTTPServer(("127.0.0.1", 0), PreviewScenario)
    worker = threading.Thread(target=server.serve_forever, daemon=True)
    worker.start()
    try:
        with tempfile.TemporaryDirectory(prefix="adrive-preview-") as directory:
            for binary in binaries:
                check_descriptor_and_raw(binary, Path(directory), f"http://127.0.0.1:{server.server_port}")
                check_failures(binary, Path(directory), f"http://127.0.0.1:{server.server_port}")
                check_preflight(binary, Path(directory), f"http://127.0.0.1:{server.server_port}")
                check_stream_limits(binary, Path(directory), f"http://127.0.0.1:{server.server_port}")
        print(f"share view regression passed for {len(binaries)} binary/binaries")
    finally:
        server.shutdown()
        server.server_close()
        worker.join()


if __name__ == "__main__":
    main()
