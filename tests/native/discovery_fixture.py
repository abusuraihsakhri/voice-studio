"""Local-only discovery fixture, never an inference backend or product service.

Run manually for native UI validation. Prints its ephemeral loopback port and
exits after 180 seconds. No dependencies, model loading, storage or uploads.
"""
from http.server import BaseHTTPRequestHandler, HTTPServer
import json
import threading


class FixtureHandler(BaseHTTPRequestHandler):
    def do_GET(self):
        payload = {
            "/health": {"status": "ok", "fixture": True},
            "/engines": {"tts": {"backends": [{"id": "fixture-only", "available": False}]}},
            "/profiles": [],
        }.get(self.path)
        if payload is None:
            self.send_error(404)
            return
        body = json.dumps(payload).encode("utf-8")
        self.send_response(200)
        self.send_header("Content-Type", "application/json")
        self.send_header("Content-Length", str(len(body)))
        self.end_headers()
        self.wfile.write(body)

    def log_message(self, *args):
        pass


if __name__ == "__main__":
    server = HTTPServer(("127.0.0.1", 0), FixtureHandler)
    print(f"Discovery fixture port: {server.server_port}", flush=True)
    timer = threading.Timer(180, server.shutdown)
    timer.daemon = True
    timer.start()
    try:
        server.serve_forever()
    finally:
        timer.cancel()
        server.server_close()
