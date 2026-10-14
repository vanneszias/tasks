"""Local OpenAI-compatible test provider; no paid models or real credentials."""
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
import json
import threading


class Model(BaseHTTPRequestHandler):
    requests = []
    def log_message(self, *args):
        pass

    def do_POST(self):
        body = json.loads(self.rfile.read(int(self.headers["Content-Length"])))
        type(self).requests.append(body)
        tools = body.get("tools", [])
        shell = next((tool["function"] for tool in tools if tool["function"]["name"] == "shell"), None)
        if shell and not any(message.get("role") == "tool" for message in body.get("messages", [])):
            params = shell.get("parameters", {}).get("properties", {})
            arguments = {"command": "echo $$ > tool-pid; echo running > started; sleep 30; echo escaped > escaped"}
            if "description" in params:
                arguments["description"] = "Disposable shutdown fixture"
            message = {"role": "assistant", "content": None, "tool_calls": [{"id": "call_fixture",
                       "type": "function", "function": {"name": "shell", "arguments": json.dumps(arguments)}}]}
            finish = "tool_calls"
        else:
            message = {"role": "assistant", "content": "fixture complete"}
            finish = "stop"
        self.send_response(200)
        if body.get("stream"):
            self.send_header("Content-Type", "text/event-stream")
            self.end_headers()
            chunk = {"id": "chatcmpl-fixture", "object": "chat.completion.chunk", "created": 1,
                     "model": "fixture", "choices": [{"index": 0, "delta": message, "finish_reason": None}]}
            if "tool_calls" in message:
                message["tool_calls"][0]["index"] = 0
            self.wfile.write(("data: " + json.dumps(chunk) + "\n\n").encode())
            chunk["choices"] = [{"index": 0, "delta": {}, "finish_reason": finish}]
            self.wfile.write(("data: " + json.dumps(chunk) + "\n\ndata: [DONE]\n\n").encode())
        else:
            self.send_header("Content-Type", "application/json")
            self.end_headers()
            self.wfile.write(json.dumps({"id": "chatcmpl-fixture", "object": "chat.completion", "created": 1,
                 "model": "fixture", "choices": [{"index": 0, "message": message, "finish_reason": finish}],
                 "usage": {"prompt_tokens": 1, "completion_tokens": 1, "total_tokens": 2}}).encode())


def start_model(root, env):
    server = ThreadingHTTPServer(("127.0.0.1", 0), Model)
    threading.Thread(target=server.serve_forever, daemon=True).start()
    env["TASKS_FIXTURE_KEY"] = "disposable-not-a-secret"
    config = {"model": "fixture/local", "snapshots": False,
              "policies": [{"action": "provider.use", "resource": "*", "effect": "deny"},
                           {"action": "provider.use", "resource": "fixture", "effect": "allow"}],
              "permissions": [{"action": "shell", "resource": "*", "effect": "allow"}],
              "providers": {"fixture": {"env": ["TASKS_FIXTURE_KEY"],
                 "package": "@opencode/ai/providers/openai-compatible",
                 "settings": {"baseURL": f"http://127.0.0.1:{server.server_port}/v1"},
                 "models": {"local": {"limit": {"context": 32768, "output": 4096},
                                      "capabilities": {"tools": True, "input": ["text"], "output": ["text"]}}}}}}
    (root / "repo/opencode.json").write_text(json.dumps(config))
    return server
