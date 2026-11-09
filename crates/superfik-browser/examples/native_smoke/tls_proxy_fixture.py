"""Loopback-only HTTP/TLS origins and authenticated HTTP/TLS proxies."""
import base64
import hashlib
import hmac
import http.client
import http.server
import json
import os
from pathlib import Path
import select
import shutil
import socket
import ssl
import subprocess
import sys
import threading
import urllib.parse

root = Path(sys.argv[1])
os.umask(0o077)
cert, key = root / "certificate.pem", root / "private-key.pem"
configuration = root / "certificate.cnf"
configuration.write_text("""[req]
distinguished_name=dn
x509_extensions=v3
prompt=no
[dn]
CN=127.0.0.1
[v3]
subjectAltName=IP:127.0.0.1
basicConstraints=critical,CA:TRUE
keyUsage=critical,digitalSignature,keyCertSign,keyEncipherment
extendedKeyUsage=serverAuth
""")
openssl = shutil.which("openssl")
subprocess.run([openssl, "req", "-x509", "-newkey", "rsa:2048", "-nodes", "-sha256",
                "-days", "1", "-config", str(configuration), "-keyout", str(key),
                "-out", str(cert)], check=True, stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
public = subprocess.check_output([openssl, "x509", "-in", str(cert), "-pubkey", "-noout"])
der = subprocess.run([openssl, "pkey", "-pubin", "-outform", "DER"], input=public,
                     stdout=subprocess.PIPE, check=True).stdout
spki = base64.b64encode(hashlib.sha256(der).digest()).decode()
tls = ssl.SSLContext(ssl.PROTOCOL_TLS_SERVER)
tls.load_cert_chain(cert, key)
origin_trust = ssl.create_default_context(cafile=str(cert))
counts = {name: 0 for name in ["origin", "direct", "authenticated", "denied", "connect", "udp"]}
lock = threading.Lock()

def count(name):
    with lock:
        counts[name] += 1

class Server(http.server.ThreadingHTTPServer):
    daemon_threads = True

    def handle_error(self, request, client_address):
        pass  # A rejected test certificate/closed browser is expected.

class Handler(http.server.BaseHTTPRequestHandler):
    protocol_version = "HTTP/1.1"

    def log_message(self, *args):
        pass  # Never log request headers or authentication.

    def body(self, status, body, kind="text/html"):
        data = body.encode() if isinstance(body, str) else body
        self.send_response(status)
        self.send_header("Content-Type", kind)
        self.send_header("Content-Length", str(len(data)))
        self.send_header("Connection", "close")
        self.end_headers()
        self.wfile.write(data)
        self.close_connection = True

udp = socket.socket(socket.AF_INET, socket.SOCK_DGRAM)
udp.bind(("127.0.0.1", 0))
udp.settimeout(.2)

def observe_udp():
    while True:
        try:
            udp.recvfrom(65536)
            count("udp")
        except socket.timeout:
            pass

threading.Thread(target=observe_udp, daemon=True).start()
fixture = (Path(__file__).parent / "proxy_fixture.html").read_text()

class Origin(Handler):
    def do_GET(self):
        with lock:
            tunneled = self.client_address[1] in tunnel_ports
        via = self.headers.get("X-Superfik-Proxy") == "yes" or tunneled
        leaked = "Proxy-Authorization" in self.headers
        count("origin")
        if not via or leaked:
            count("direct")
        if self.path == "/worker":
            self.body(200, json.dumps({"via": via and not leaked}), "application/json")
        else:
            self.body(200, fixture.replace("__VIA__", str(via and not leaked).lower())
                      .replace("__STUN_PORT__", str(udp.getsockname()[1])))

allowed_ports = set()

class Proxy(Handler):
    def authorized(self):
        expected = "Basic " + base64.b64encode(b"lab:lab-password").decode()
        supplied = self.headers.get("Proxy-Authorization", "")
        if hmac.compare_digest(supplied, expected):
            count("authenticated")
            return True
        count("denied")
        self.send_response(407)
        self.send_header("Proxy-Authenticate", 'Basic realm="Superfik isolated fixture"')
        self.send_header("Content-Length", "0")
        self.send_header("Connection", "close")
        self.end_headers()
        self.close_connection = True
        return False

    def allowed(self, host, port):
        return host == "127.0.0.1" and port in allowed_ports

    def do_GET(self):
        target = urllib.parse.urlsplit(self.path)
        if not self.allowed(target.hostname, target.port) or target.scheme not in ("http", "https"):
            self.body(403, "Fixture only forwards to its own loopback origins")
            return
        if not self.authorized():
            return
        cls = http.client.HTTPSConnection if target.scheme == "https" else http.client.HTTPConnection
        options = {"context": origin_trust} if target.scheme == "https" else {}
        connection = cls(target.hostname, target.port, timeout=5, **options)
        connection.request("GET", urllib.parse.urlunsplit(("", "", target.path or "/", target.query, "")),
                           headers={"X-Superfik-Proxy": "yes"})
        response = connection.getresponse()
        self.body(response.status, response.read(), response.getheader("Content-Type", "text/plain"))
        connection.close()

    def do_CONNECT(self):
        target = urllib.parse.urlsplit("//" + self.path)
        if not self.allowed(target.hostname, target.port):
            self.body(403, "Fixture only tunnels to its own loopback origins")
            return
        if not self.authorized():
            return
        count("connect")
        with socket.create_connection((target.hostname, target.port), timeout=5) as upstream:
            with lock:
                tunnel_ports.add(upstream.getsockname()[1])
            self.send_response(200, "Connection Established")
            self.end_headers()
            self.wfile.flush()
            endpoints = [self.connection, upstream]
            try:
                while True:
                    ready = [s for s in endpoints if isinstance(s, ssl.SSLSocket) and s.pending()]
                    if not ready:
                        ready, _, _ = select.select(endpoints, [], [], 5)
                    if not ready:
                        break
                    for source in ready:
                        data = source.recv(65536)
                        if not data:
                            return
                        (upstream if source is self.connection else self.connection).sendall(data)
            except (OSError, ssl.SSLError):
                pass
            self.close_connection = True

tunnel_ports = set()

def start(handler, secure=False):
    server = Server(("127.0.0.1", 0), handler)
    if secure:
        server.socket = tls.wrap_socket(server.socket, server_side=True)
    threading.Thread(target=server.serve_forever, daemon=True).start()
    return server

http_origin, https_origin = start(Origin), start(Origin, True)
allowed_ports.update([http_origin.server_port, https_origin.server_port])
http_proxy, https_proxy = start(Proxy), start(Proxy, True)
print(json.dumps({"http_origin": f"http://127.0.0.1:{http_origin.server_port}/",
                  "https_origin": f"https://127.0.0.1:{https_origin.server_port}/",
                  "http_proxy": http_proxy.server_port, "https_proxy": https_proxy.server_port,
                  "spki": spki}), flush=True)
for line in sys.stdin:
    if line.strip() == "quit":
        break
    if line.strip() == "stats":
        with lock:
            print(json.dumps(counts), flush=True)
