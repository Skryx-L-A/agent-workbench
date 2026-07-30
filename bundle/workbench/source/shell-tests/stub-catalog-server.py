#!/usr/bin/env python3
# stub-catalog-server.py — a throwaway local HTTP server for test-model-catalog.sh.
#
# Never a real provider in a test (rule 2026-07-29): this binds 127.0.0.1 on an
# OS-assigned free port and serves whatever JSON the test drops into RESP_DIR, keyed
# by URL path. It also logs every request path + auth-header value it saw, so the
# test can prove a key was actually SENT to the (fake) provider without that key ever
# appearing in any file wb-state itself writes.
#
# Usage: stub-catalog-server.py <RESP_DIR> <LOG_FILE> <PORT_FILE>
#   RESP_DIR/status_<path-with-slashes-as-underscores>   optional, HTTP status (default 200)
#   RESP_DIR/body_<path-with-slashes-as-underscores>.json  response body (missing = 404)
# Prints nothing; writes the bound port to PORT_FILE once listening.
import http.server
import os
import sys

RESP_DIR, LOG_FILE, PORT_FILE = sys.argv[1:4]


class Handler(http.server.BaseHTTPRequestHandler):
    def do_GET(self):
        auth = (self.headers.get("Authorization") or self.headers.get("x-api-key")
                or self.headers.get("x-goog-api-key") or "")
        with open(LOG_FILE, "a") as f:
            f.write("%s\t%s\n" % (self.path, auth))
        key = self.path.replace("/", "_")
        status_file = os.path.join(RESP_DIR, "status" + key)
        body_file = os.path.join(RESP_DIR, "body" + key + ".json")
        code = 200
        if os.path.exists(status_file):
            code = int(open(status_file).read().strip())
        if code != 200:
            self.send_response(code)
            self.end_headers()
            return
        if not os.path.exists(body_file):
            self.send_response(404)
            self.end_headers()
            return
        data = open(body_file, "rb").read()
        self.send_response(200)
        self.send_header("Content-Type", "application/json")
        self.send_header("Content-Length", str(len(data)))
        self.end_headers()
        self.wfile.write(data)

    def log_message(self, *args):
        pass


srv = http.server.HTTPServer(("127.0.0.1", 0), Handler)
with open(PORT_FILE, "w") as f:
    f.write(str(srv.server_address[1]))
srv.serve_forever()
