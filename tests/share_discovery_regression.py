"""MCP discovery and executed download planning through real entrypoints, localhost only."""
import http.server
import json
import os
from pathlib import Path
import queue
import subprocess
import sys
import tempfile
import threading

from share_dry_run_regression import PlanFixture


class Rpc:
    def __init__(self, binary, root):
        prefix = [] if binary.stem == "ve-adrive-cli" else ["ve-adrive"]
        environment = {key: value for key, value in os.environ.items() if not key.startswith("ADRIVE_")}
        self.process = subprocess.Popen([str(binary), *prefix, "serve", "--mcp", "--transport", "stdio",
            "--config-path", str(root / "config"), "--credentials-path", str(root / "credentials")],
            stdin=subprocess.PIPE, stdout=subprocess.PIPE, stderr=subprocess.PIPE, text=True, env=environment)
        self.messages = queue.Queue()
        self.reader = threading.Thread(target=self.read, daemon=True)
        self.reader.start()
        try:
            self.call(1, "initialize", {"protocolVersion": "2024-11-05", "capabilities": {},
                                       "clientInfo": {"name": "share-regression", "version": "1"}})
            self.send({"jsonrpc": "2.0", "method": "notifications/initialized"})
        except BaseException:
            self.close()
            raise

    def read(self):
        for line in self.process.stdout:
            self.messages.put(json.loads(line))

    def send(self, value):
        self.process.stdin.write(json.dumps(value) + "\n")
        self.process.stdin.flush()

    def call(self, identifier, method, params):
        self.send({"jsonrpc": "2.0", "id": identifier, "method": method, "params": params})
        while True:
            message = self.messages.get(timeout=15)
            if message.get("id") == identifier:
                assert "error" not in message, message
                return message["result"]

    def close(self):
        self.process.terminate()
        self.process.communicate(timeout=5)
        self.reader.join(timeout=5)


def payload(result):
    assert not result.get("isError"), result
    return json.loads(result["content"][0]["text"])


def check_rpc(binary, root, origin):
    rpc = Rpc(binary, root)
    try:
        tool = next(item for item in rpc.call(2, "tools/list", {})["tools"]
                    if item["name"] == "ve_adrive_share_download")
        assert tool["annotations"]["readOnlyHint"] is False
        assert tool["annotations"]["destructiveHint"] is False
        assert "metadata" in tool["description"] and "download quota" in tool["description"]
        arguments = {"share-url": f"{origin}/share/s", "destination-dir": str(root / "planned"),
                     "path": "tree/", "recursive": True}
        before = len(PlanFixture.requests)
        planned = payload(rpc.call(3, "tools/call", {"name": tool["name"], "arguments": arguments}))
        assert planned["execution_status"] == "planned_not_executed" and len(PlanFixture.requests) == before
        result = payload(rpc.call(4, "tools/call", {"name": tool["name"],
                         "arguments": {**arguments, "execute": True, "dry_run": True}}))
        assert result["exit_code"] == 0, result
        selection = json.loads(result["stdout"])["data"]["selection"]
        assert selection["FileCount"] == 3 and selection["TotalBytes"] == 9
        assert selection["LocalWrites"] is False and not (root / "planned").exists()
        assert not any("download_url" in path or path == "/content" for path in PlanFixture.requests[before:])
        assert not (root / "config").exists() and not (root / "credentials").exists()
        check_files_rpc(rpc, root, origin)
    finally:
        rpc.close()


def check_files_rpc(rpc, root, origin):
    tool = next(item for item in rpc.call(5, "tools/list", {})["tools"]
                if item["name"] == "ve_adrive_share_files")
    properties = tool["inputSchema"]["properties"]
    assert "limit" not in properties and properties["max-keys"]["default"] == 1000
    arguments = {"share-url": f"{origin}/share/s", "path": "tree/", "max-keys": 3,
                 "execute": True}
    result = payload(rpc.call(6, "tools/call", {"name": tool["name"], "arguments": arguments}))
    assert result["exit_code"] == 0, result
    envelope = json.loads(result["stdout"])
    assert envelope["data"]["Selection"]["MaxKeys"] == 3
    assert envelope["pagination"]["total_returned"] == 3
    assert not any(path.name not in {"config", "credentials"} for path in root.iterdir())
    before = len(PlanFixture.requests)
    rejected = rpc.call(7, "tools/call", {"name": tool["name"],
                       "arguments": {"share-url": f"{origin}/share/s", "limit": 1, "execute": True}})
    assert rejected.get("isError") and len(PlanFixture.requests) == before, rejected


def main():
    binaries = [Path(value).resolve() for value in sys.argv[1:]]
    assert binaries, "pass the unified and standalone binary paths"
    server = http.server.ThreadingHTTPServer(("127.0.0.1", 0), PlanFixture)
    worker = threading.Thread(target=server.serve_forever, daemon=True)
    worker.start()
    try:
        for binary in binaries:
            with tempfile.TemporaryDirectory(prefix="share-discovery-") as directory:
                check_rpc(binary, Path(directory), f"http://127.0.0.1:{server.server_port}")
        print(f"PASS: {len(binaries)} entrypoints; MCP annotations, offline argv plan and executed read-only metadata plan")
    finally:
        server.shutdown()
        server.server_close()
        worker.join()


if __name__ == "__main__":
    main()
