"""Actual-entrypoint share download aggregate and dry-run regression fixtures.

Run: python3 tests/share_download_regression.py [unified-binary] [standalone-binary]
"""
import http.server
import json
import signal
import subprocess
import sys
import tempfile
import threading
from pathlib import Path
from urllib.parse import parse_qs, urlsplit


class Receiver(http.server.BaseHTTPRequestHandler):
    requests = []
    content_started = threading.Event()
    is_stalled = False
    issuance_status = 200
    is_valid_content = False
    issued_address = None
    token_status = 200

    def log_message(self, *_arguments):
        pass

    def respond(self, payload, status=200):
        encoded = json.dumps(payload).encode()
        self.send_response(status)
        self.send_header("Content-Length", str(len(encoded)))
        self.end_headers()
        self.wfile.write(encoded)

    def do_POST(self):
        self.rfile.read(int(self.headers.get("Content-Length", "0")))
        self.requests.append(self.path)
        if self.token_status == 403:
            self.respond({"Code": "Forbidden.ShareDownloadDisabled"}, 403)
        else:
            self.respond({"AccessToken": "fixture-token", "ExpiresIn": 60})

    def do_GET(self):
        self.requests.append(self.path)
        if self.path.startswith("/v1/share_link/get_by_anonymous"):
            self.respond({"ShareID": "s"})
        elif self.path.startswith("/v1/share_file/get_download_url"):
            if self.issuance_status != 200:
                encoded = b'{"Code":"Forbidden.ShareDownloadDisabled"}'
                self.send_response(self.issuance_status)
                self.send_header("Content-Length", str(len(encoded)))
                self.send_header("x-ids-request-id", "explicit-denial-id")
                self.end_headers()
                self.wfile.write(encoded)
            else:
                address = self.issued_address if self.issued_address is not None else f"http://127.0.0.1:{self.server.server_port}/content?secret=signature"
                self.respond({"DownloadURL": address})
        elif self.path.startswith("/v1/share_file/get?"):
            selected = parse_qs(urlsplit(self.path).query)["file_path"][0]
            self.respond({"FilePath": selected, "Size": 3, "ETag": "abc"})
        elif self.path.startswith("/content"):
            assert not self.headers.get("Authorization")
            assert not self.headers.get("x-share-token")
            self.content_started.set()
            if self.is_valid_content:
                self.send_response(200)
                self.send_header("Content-Length", "3")
                self.send_header("ETag", "abc")
                self.end_headers()
                self.wfile.write(b"abc")
                return
            if self.is_stalled:
                self.send_response(200)
                self.send_header("Content-Length", "3")
                self.send_header("ETag", "abc")
                self.end_headers()
                threading.Event().wait(1)
                return
            self.respond("wrong-size")
        else:
            self.respond({}, 404)


def check_binary(binary, prefix, server):
    with tempfile.TemporaryDirectory(prefix="share-cli-", dir=Path(tempfile.gettempdir()).resolve()) as folder:
        root = Path(folder)
        url = f"http://127.0.0.1:{server.server_port}/share/s?pwd=fixture-password"
        common = [str(binary), *prefix, "share", "download", url, str(root / "out"),
                  "--path", "a", "--checkpoint", str(root / "journal"), "--output", "json",
                  "--config-path", str(root / "missing-config"),
                  "--credentials-path", str(root / "missing-credentials")]
        before = len(Receiver.requests)
        dry = subprocess.run([*common, "--dry-run"], capture_output=True, timeout=10)
        assert dry.returncode == 0 and json.loads(dry.stdout)["success"]
        assert len(Receiver.requests) == before
        assert not (root / "out").exists() and not (root / "journal").exists()
        failed = subprocess.run(common, capture_output=True, timeout=10)
        assert failed.returncode != 0
        envelope = json.loads(failed.stdout)  # A second envelope would fail JSON parsing.
        assert envelope["success"] is False and envelope["request_id"] is None
        assert envelope["data"]["PlanComplete"] is True
        assert envelope["data"]["Summary"]["Failed"] == 1
        assert envelope["data"]["CheckpointAvailable"] is True
        assert envelope["error"]["exit_code"] == failed.returncode
        assert not (root / "out/a").exists()
        for secret in (b"fixture-password", b"fixture-token", b"secret=signature"):
            assert secret not in failed.stdout + failed.stderr
            assert secret not in (root / "journal").read_bytes()


def check_interrupt(binary, prefix, server):
    with tempfile.TemporaryDirectory(prefix="share-interrupt-", dir=Path(tempfile.gettempdir()).resolve()) as folder:
        root = Path(folder)
        Receiver.content_started.clear()
        Receiver.is_stalled = True
        arguments = [str(binary), *prefix, "share", "download",
                     f"http://127.0.0.1:{server.server_port}/share/s", str(root / "out"),
                     "--path", "a", "--checkpoint", str(root / "journal"), "--output", "json",
                     "--config-path", str(root / "missing-config"),
                     "--credentials-path", str(root / "missing-credentials")]
        process = subprocess.Popen(arguments, stdout=subprocess.PIPE, stderr=subprocess.PIPE)
        try:
            assert Receiver.content_started.wait(5), "content did not start"
            process.send_signal(signal.SIGINT)
            stdout, _stderr = process.communicate(timeout=5)
            envelope = json.loads(stdout)
            assert process.returncode != 0 and not envelope["success"]
            assert envelope["data"]["PlanComplete"] and envelope["data"]["CheckpointAvailable"]
            assert not (root / "out/a").exists()
            assert json.loads((root / "journal").read_text())["completed"] == [None]
        finally:
            Receiver.is_stalled = False
            if process.poll() is None:
                process.kill()
                process.wait()


def check_selector_tamper(binary, prefix, server):
    with tempfile.TemporaryDirectory(prefix="share-selector-", dir=Path(tempfile.gettempdir()).resolve()) as folder:
        root = Path(folder)
        arguments = [str(binary), *prefix, "share", "download",
                     f"http://127.0.0.1:{server.server_port}/share/s", str(root / "out"),
                     "--path", "d/a", "--checkpoint", str(root / "journal"), "--output", "json",
                     "--config-path", str(root / "missing-config"),
                     "--credentials-path", str(root / "missing-credentials")]
        pending = subprocess.run(arguments, capture_output=True, timeout=10)
        assert pending.returncode != 0 and json.loads(pending.stdout)["data"]["CheckpointAvailable"]
        journal = json.loads((root / "journal").read_text())
        assert journal["manifest"]["roots"] == ["d/a"]
        journal["manifest"]["roots"] = ["d/"]
        journal["manifest"]["files"][0]["local"] = "d/a"
        (root / "journal").write_text(json.dumps(journal))
        signed_before = sum("get_download_url" in request for request in Receiver.requests)
        resumed = subprocess.run([*arguments, "--resume"], capture_output=True, timeout=10)
        assert resumed.returncode != 0 and not json.loads(resumed.stdout)["success"]
        assert not (root / "out/d").exists()
        assert sum("get_download_url" in request for request in Receiver.requests) == signed_before


def check_known_denial(binary, prefix, server):
    with tempfile.TemporaryDirectory(prefix="share-denial-") as folder:
        root = Path(folder)
        Receiver.issuance_status = 403
        before = sum("get_download_url" in request for request in Receiver.requests)
        try:
            result = subprocess.run([str(binary), *prefix, "share", "download",
                f"http://127.0.0.1:{server.server_port}/share/s", str(root / "out"),
                "--path", "a", "--output", "json"], capture_output=True, timeout=10)
            envelope = json.loads(result.stdout)
            assert result.returncode == 5 and envelope["status_code"] == 403
            assert envelope["ec"] == "Forbidden.ShareDownloadDisabled"
            assert envelope["error"]["kind"] == "permission_denied"
            assert envelope["error"]["category"] == "AuthError"
            assert envelope["request_id"] == "explicit-denial-id"
            assert envelope["data"]["IssuanceOutcomeUnknown"] is False
            assert envelope["data"]["Summary"]["Failed"] == 1
            assert not (root / "out/a").exists()
            assert sum("get_download_url" in request for request in Receiver.requests) == before + 1
        finally:
            Receiver.issuance_status = 200


def check_denial_tabular_output(binary, prefix, server):
    with tempfile.TemporaryDirectory(prefix="share-view-display-") as folder:
        root = Path(folder)
        arguments = [str(binary), *prefix, "share", "download",
                     f"http://127.0.0.1:{server.server_port}/share/s", str(root / "out"),
                     "--path", "a", "--config-path", str(root / "missing-config"),
                     "--credentials-path", str(root / "missing-credentials")]
        try:
            for has_plan, output in ((False, "table"), (False, "csv"), (True, "table"), (True, "csv")):
                Receiver.token_status = 200 if has_plan else 403
                Receiver.issuance_status = 403
                result = subprocess.run([*arguments, "--output", output], capture_output=True, text=True, timeout=10)
                assert result.returncode == 5
                assert "Forbidden.ShareDownloadDisabled" in result.stdout, "tabular output hid the failure"
                assert "Share download is disabled" in result.stdout
                expected = ("data.summary", "data.complete") if output == "table" else ("data.Summary", "data.Complete")
                assert all(field in result.stdout for field in expected)
                assert not (root / "out/a").exists()
        finally:
            Receiver.token_status = 200
            Receiver.issuance_status = 200


def check_root_alias_resume(binary, prefix, server):
    with tempfile.TemporaryDirectory(prefix="share-root-") as folder:
        root = Path(folder)
        (root / "actual").mkdir()
        (root / "alias").symlink_to(root / "actual", target_is_directory=True)
        arguments = [str(binary), *prefix, "share", "download",
            f"http://127.0.0.1:{server.server_port}/share/s", str(root / "alias/out"),
            "--path", "a", "--checkpoint", str(root / "alias/journal"), "--output", "json"]
        Receiver.is_valid_content = True
        try:
            result = subprocess.run(arguments, capture_output=True, timeout=10)
            assert result.returncode == 0, result.stderr.decode()
            assert (root / "actual/out/a").read_bytes() == b"abc"
            journal = json.loads((root / "actual/journal").read_text())
            assert journal["identity"]["destination"] == str((root / "actual/out").resolve())
            arguments[len(prefix) + 4] = str((root / "actual/out").resolve())
            arguments[arguments.index("--checkpoint") + 1] = str((root / "actual/journal").resolve())
            before = sum("get_download_url" in request for request in Receiver.requests)
            resumed = subprocess.run([*arguments, "--resume"], capture_output=True, timeout=10)
            assert resumed.returncode == 0 and json.loads(resumed.stdout)["data"]["Summary"]["Completed"] == 1
            assert sum("get_download_url" in request for request in Receiver.requests) == before
        finally:
            Receiver.is_valid_content = False


def check_unusable_address(binary, prefix, server):
    with tempfile.TemporaryDirectory(prefix="share-address-") as folder:
        root = Path(folder)
        try:
            for address in ("", "ftp://example.test/secret", "https://user:secret@example.test/blob"):
                Receiver.issued_address = address
                before = len(Receiver.requests)
                result = subprocess.run([str(binary), *prefix, "share", "download",
                    f"http://127.0.0.1:{server.server_port}/share/s", str(root / "out"),
                    "--path", "a", "--output", "json"], capture_output=True, timeout=10)
                envelope = json.loads(result.stdout)
                assert result.returncode == 8 and envelope["ec"] == "share_download_url_result_unknown"
                assert envelope["data"]["IssuanceOutcomeUnknown"] is True
                assert envelope["request_id"] is None
                assert "Do not automatically retry" in envelope["error"]["suggested_action"]
                assert b"secret" not in result.stdout + result.stderr
                assert not (root / "out/a").exists()
                requests = Receiver.requests[before:]
                assert len(requests) == 4 and sum("get_download_url" in request for request in requests) == 1
        finally:
            Receiver.issued_address = None


def main():
    repository = Path(__file__).resolve().parent.parent
    binaries = [Path(argument) for argument in sys.argv[1:]] or [
        repository / "target/debug/ve-storage-uni-cli", repository / "packaging/cargo/ve-adrive-cli/target/debug/ve-adrive-cli"]
    server = http.server.ThreadingHTTPServer(("127.0.0.1", 0), Receiver)
    worker = threading.Thread(target=server.serve_forever, daemon=True)
    worker.start()
    try:
        for binary in binaries:
            prefix = [] if binary.name == "ve-adrive-cli" else ["ve-adrive"]
            check_binary(binary, prefix, server)
            check_interrupt(binary, prefix, server)
            check_selector_tamper(binary, prefix, server)
            check_known_denial(binary, prefix, server)
            check_denial_tabular_output(binary, prefix, server)
            check_root_alias_resume(binary, prefix, server)
            check_unusable_address(binary, prefix, server)
        print(f"PASS: {len(binaries)} CLI entrypoints; dry-run, failure aggregate, SIGINT, selector safety, definite 403/table/CSV diagnostics, root-alias resume and unusable-address ambiguity")
    finally:
        server.shutdown()
        server.server_close()
        worker.join()


if __name__ == "__main__":
    main()
