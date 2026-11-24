"""Fremde Adresse als Zaehler: ein Listener auf 127.0.0.1, der jede aufgebaute Verbindung zaehlt.

Das ist die Netzgrenze von S2. Der Laeufer geht in der Vorfuehrung nie ins Netz nach aussen; die „fremde Adresse“ ist ein
Ziel, das der Einschluss nicht erreichen darf und das ausserhalb des Einschlusses antwortet.
"""
from __future__ import annotations

import socket
import threading


class Fremd:
    def __init__(self, bind=("127.0.0.1", 0)):
        self.srv = socket.socket(socket.AF_INET, socket.SOCK_STREAM)
        self.srv.setsockopt(socket.SOL_SOCKET, socket.SO_REUSEADDR, 1)
        self.srv.bind(bind)
        self.srv.listen(16)
        self.port = self.srv.getsockname()[1]
        self.verbindungen = 0
        self.lock = threading.Lock()
        self.offen = True
        threading.Thread(target=self._lauf, daemon=True).start()

    def url(self):
        return "http://127.0.0.1:%d/" % self.port

    def _lauf(self):
        while True:
            try:
                c, _ = self.srv.accept()
            except OSError:
                return
            with self.lock:
                self.verbindungen += 1
            try:
                c.settimeout(2)
                c.recv(4096)
                c.sendall(b"HTTP/1.0 200 OK\r\nContent-Length: 5\r\n\r\nfremd")
            except OSError:
                pass
            finally:
                c.close()

    def zaehler(self):
        with self.lock:
            return self.verbindungen

    def schliessen(self):
        self.offen = False
        try:
            self.srv.shutdown(socket.SHUT_RDWR)  # unter Linux haelt ein blockiertes accept() den Listener offen, close() allein genuegt nicht
        except OSError:
            pass
        self.srv.close()
