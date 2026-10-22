"""Share-page endpoint rename, precedence and discovery through real CLI entrypoints."""
import http.server
import json
import os
from pathlib import Path
import subprocess
import sys
import tempfile
import threading

from share_discovery_regression import Rpc, payload


class Management(http.server.BaseHTTPRequestHandler):
    def log_message(self, *_arguments):
        pass

    def do_GET(self):
        assert self.path.startswith("/v1/share_link/get?"), self.path
        encoded = json.dumps({"ShareID": "s", "InstanceID": "inst"}).encode()
        self.send_response(200)
        self.send_header("Content-Length", str(len(encoded)))
        self.end_headers()
        self.wfile.write(encoded)


def invoke(binary, root, arguments, overrides=None):
    prefix = [] if binary.stem == "ve-adrive-cli" else ["ve-adrive"]
    environment = {key: value for key, value in os.environ.items() if not key.startswith("ADRIVE_")}
    environment.update(overrides or {})
    result = subprocess.run([str(binary), *prefix, "--config-path", str(root / "config"),
        "--credentials-path", str(root / "credentials"), "--output", "json", *arguments],
        env=environment, capture_output=True, text=True, timeout=15)
    assert "must-not-echo" not in result.stdout + result.stderr
    return result


def configure(root, api, key=None, origin=None):
    contents = f"[default.adrive]\nauth_mode='aksk'\nendpoint='{api}'\nregion='cn-beijing'\n"
    if key:
        contents += f"{key}='{origin}'\n"
    (root / "config").write_text(contents)
    # Explicit credentials-path uses its own credential store, even with a legacy config.
    (root / "credentials").write_text("schema_version=1\n[default.adrive]\n"
                                      "access_key_id='fixture-ak'\nsecret_access_key='fixture-sk'\n")


def check_origins(binary, root, api):
    command = ["share", "get", "adrive://inst/s", "--auth-mode", "aksk"]
    canonical = "http://canonical.test:8080"
    legacy = "https://legacy.test"
    for environment, expected in [
        ({"ADRIVE_SHARE_ENDPOINT": canonical}, canonical),
        ({"ADRIVE_SHARE_BASE_URL": legacy}, legacy),
        ({"ADRIVE_SHARE_ENDPOINT": canonical, "ADRIVE_SHARE_BASE_URL": legacy}, canonical),
    ]:
        configure(root, api)
        result = invoke(binary, root, command, environment)
        assert result.returncode == 0, result.stderr
        assert json.loads(result.stdout)["data"]["ShareUrl"] == expected + "/share/s"
    for key in ("share_endpoint", "share_base_url"):
        configure(root, api, key, legacy)
        result = invoke(binary, root, command, {"ADRIVE_SHARE_ENDPOINT": canonical})
        assert result.returncode == 0, result.stderr
        assert json.loads(result.stdout)["data"]["ShareUrl"] == legacy + "/share/s"
        for flag in ("--share-endpoint", "--share-base-url"):
            result = invoke(binary, root, [*command, flag, canonical])
            assert result.returncode == 0, result.stderr
            assert json.loads(result.stdout)["data"]["ShareUrl"] == canonical + "/share/s"
    configure(root, api)
    result = invoke(binary, root, command,
                    {"ADRIVE_SHARE_ENDPOINT": "https://user:must-not-echo@pages.test"})
    assert result.returncode == 6, result


def check_config(binary, root, api):
    configure(root, api, "share_base_url", "http://legacy-config.test")
    for key in ("share_endpoint", "share_base_url", "default.adrive.share_base_url"):
        command = ["config", "set", key, "http://configured.test"]
        before = (root / "config").read_bytes()
        result = invoke(binary, root, [*command, "--dry-run"])
        assert result.returncode == 0, result.stderr
        assert "share_endpoint" in result.stdout and "share_base_url" not in result.stdout
        assert (root / "config").read_bytes() == before
        result = invoke(binary, root, command)
        assert result.returncode == 0, result.stderr
        saved = json.loads(result.stdout)["data"]
        assert saved["field"] == "share_endpoint" and saved["key"].endswith("share_endpoint")
        contents = (root / "config").read_text()
        assert 'share_endpoint = "http://configured.test"' in contents and "share_base_url" not in contents
    result = invoke(binary, root, ["config", "show"])
    assert result.returncode == 0, result.stderr
    assert "share_endpoint" in result.stdout and "share_base_url" not in result.stdout


def check_discovery(binary, root):
    for action, selector in (("create", "adrive://inst/space/file"), ("get", "adrive://inst/s"),
                             ("list", "adrive://inst"), ("update", "adrive://inst/s")):
        for language in ("en", "zh"):
            result = invoke(binary, root, ["share", action, "--help", "--help-language", language])
            assert result.returncode == 0, result
            assert "--share-endpoint" in result.stdout and "--share-base-url" not in result.stdout
        result = invoke(binary, root, ["share", action, selector, "--describe"])
        assert result.returncode == 0, result.stderr
        parameters = json.loads(result.stdout)["data"]["parameters"]
        assert any(item["name"] == "share-endpoint" for item in parameters)
        assert all(item["name"] != "share-base-url" for item in parameters)
    rpc = Rpc(binary, root)
    try:
        tool = next(item for item in rpc.call(20, "tools/list", {})["tools"]
                    if item["name"] == "ve_adrive_share_get")
        properties = tool["inputSchema"]["properties"]
        assert "share-endpoint" in properties and "share-base-url" not in properties
        result = payload(rpc.call(21, "tools/call", {"name": tool["name"], "arguments": {
            "share-id": "adrive://inst/s", "share-endpoint": "http://mcp-pages.test",
            "execute": True}}))
        assert result["exit_code"] == 0, result
        assert json.loads(result["stdout"])["data"]["ShareUrl"] == "http://mcp-pages.test/share/s"
    finally:
        rpc.close()


def main():
    binaries = [Path(value).resolve() for value in sys.argv[1:]]
    assert binaries, "pass the unified and standalone binary paths"
    server = http.server.ThreadingHTTPServer(("127.0.0.1", 0), Management)
    threading.Thread(target=server.serve_forever, daemon=True).start()
    try:
        api = f"http://127.0.0.1:{server.server_port}"
        for binary in binaries:
            with tempfile.TemporaryDirectory(prefix="share-endpoint-") as directory:
                root = Path(directory)
                check_origins(binary, root, api)
                check_config(binary, root, api)
                check_discovery(binary, root)
        print(f"PASS: {len(binaries)} entrypoints; canonical/legacy endpoint, precedence, config, help and MCP")
    finally:
        server.shutdown()
        server.server_close()


if __name__ == "__main__":
    main()
