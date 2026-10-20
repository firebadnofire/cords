#!/usr/bin/env python3
"""Run-scoped TLS 1.3 TCP proxy, including unmodified WebSocket byte streams."""
import argparse
import socket
import socketserver
import ssl
import threading


def serve(port, target, certificate, key):
    context = ssl.SSLContext(ssl.PROTOCOL_TLS_SERVER)
    context.minimum_version = ssl.TLSVersion.TLSv1_3
    context.load_cert_chain(certificate, key)

    class Handler(socketserver.BaseRequestHandler):
        def handle(self):
            try:
                client = context.wrap_socket(self.request, server_side=True)
            except ssl.SSLError as exc:
                print('TLS rejection:', exc, flush=True)
                return
            with client, socket.create_connection(('127.0.0.1', target), timeout=10) as backend:
                backend.settimeout(None)

                def copy(source, destination):
                    try:
                        while data := source.recv(65536):
                            destination.sendall(data)
                    except (OSError, ssl.SSLError) as exc:
                        print('proxy stream closed:', type(exc).__name__, flush=True)
                    finally:
                        try:
                            destination.shutdown(socket.SHUT_RDWR)
                        except OSError:
                            pass  # Already closed by its peer.

                reader = threading.Thread(target=copy, args=(client, backend), daemon=True)
                reader.start()
                copy(backend, client)
                reader.join(timeout=5)

    class Server(socketserver.ThreadingTCPServer):
        allow_reuse_address = True
        daemon_threads = True

    with Server(('192.168.86.54', port), Handler) as server:
        server.serve_forever()


if __name__ == '__main__':
    p = argparse.ArgumentParser()
    p.add_argument('port', type=int)
    p.add_argument('target', type=int)
    p.add_argument('certificate')
    p.add_argument('key')
    a = p.parse_args()
    serve(a.port, a.target, a.certificate, a.key)
