#!/usr/bin/env python3
"""g9toy_reportd: a toy line-protocol report service for the G9 week 1 lab.

Synthetic. It serves synthetic text files from one directory, keeps a counter in
another, and answers two questions about itself: which identity it runs as, and
whether it can create a file where its configuration says it may try. It has no
authentication on purpose: access control in this lab is the operating system's job.

Protocol: one request per line, one reply per request.
  LIST          names of the report files
  GET <name>    the content of one report file (name: lower case, digits, underscore, ".txt")
  COUNT         increment and return the request counter kept in state_dir
  WHOAMI        identity lines of this process (user, group, capability, sandbox flags)
  SELFTEST      create and remove one marker file in scratch_dir, and report the outcome
  QUIT          close the connection
A reply starts with OK or ERR. An ERR line names the errno and its symbolic name.
"""
import argparse
import errno
import ipaddress
import os
import re
import socketserver
import sys

NAME_RE = re.compile(r"^[a-z0-9_]{1,32}\.txt$")
MAX_LINE = 256
MAX_BODY = 4096
MAX_COMMANDS = 100
CONFIG_KEYS = ("bind", "port", "reports_dir", "state_dir", "scratch_dir")
STATUS_FIELDS = ("Uid", "Gid", "Groups", "NoNewPrivs", "CapEff", "Seccomp")


def parse_config(text):
    """Parse key = value lines ('#' starts a comment). Unknown keys and bad values raise ValueError."""
    cfg = {}
    for number, raw in enumerate(text.splitlines(), start=1):
        line = raw.split("#", 1)[0].strip()
        if not line:
            continue
        key, sep, value = line.partition("=")
        key, value = key.strip(), value.strip()
        if not sep or key not in CONFIG_KEYS or not value:
            raise ValueError(f"line {number}: expected 'key = value' with a known key")
        cfg[key] = value
    missing = [key for key in CONFIG_KEYS if key not in cfg]
    if missing:
        raise ValueError("missing keys: " + ", ".join(missing))
    try:
        address = ipaddress.ip_address(cfg["bind"])
    except ValueError as exc:
        raise ValueError("bind must be an IPv4 address") from exc
    if address.version != 4:
        raise ValueError("bind must be an IPv4 address")
    if not cfg["port"].isdigit() or not 1024 <= int(cfg["port"]) <= 65535:
        raise ValueError("port must be a number from 1024 to 65535")
    cfg["port"] = int(cfg["port"])
    for key in ("reports_dir", "state_dir", "scratch_dir"):
        if not os.path.isabs(cfg[key]):
            raise ValueError(f"{key} must be an absolute path")
    return cfg


def printable(text, limit=60):
    """Log-safe form of untrusted text: printable ASCII only, cut to `limit` characters."""
    return re.sub(r"[^\x20-\x7e]", "?", text)[:limit]


def err(exc):
    """One ERR reply for an OSError."""
    return f"ERR {exc.errno} {errno.errorcode.get(exc.errno, 'EUNKNOWN')}"


def read_status_fields(path="/proc/self/status"):
    """The identity lines of a process status file; an empty dict when it cannot be read."""
    out = {}
    try:
        with open(path, encoding="ascii", errors="replace") as handle:
            for line in handle:
                key, _sep, value = line.partition(":")
                if key in STATUS_FIELDS:
                    out[key] = " ".join(value.split())
    except OSError:
        pass
    return out


def cmd_list(cfg):
    try:
        names = sorted(name for name in os.listdir(cfg["reports_dir"]) if NAME_RE.match(name))
    except OSError as exc:
        return err(exc)
    return "OK " + str(len(names)) + "".join("\n" + name for name in names)


def cmd_get(cfg, name):
    if not NAME_RE.match(name):
        return "ERR 400 bad name"
    path = os.path.join(cfg["reports_dir"], name)
    try:
        fd = os.open(path, os.O_RDONLY | os.O_NOFOLLOW)
        try:
            data = os.read(fd, MAX_BODY)
        finally:
            os.close(fd)
    except OSError as exc:
        return err(exc)
    return f"OK {len(data)}\n" + data.decode("utf-8", errors="replace").rstrip("\n")


def cmd_count(cfg):
    path = os.path.join(cfg["state_dir"], "counter")
    try:
        try:
            with open(path, "r", encoding="ascii") as handle:
                value = int(handle.read().strip() or "0")
        except FileNotFoundError:
            value = 0
        value += 1
        fd = os.open(path, os.O_WRONLY | os.O_CREAT | os.O_TRUNC, 0o640)
        with os.fdopen(fd, "w", encoding="ascii") as handle:
            handle.write(f"{value}\n")
    except OSError as exc:
        return err(exc)
    except ValueError:
        return "ERR 500 bad counter file"
    return f"OK {value}"


def cmd_whoami():
    fields = read_status_fields()
    if not fields:
        return "ERR 500 status unavailable"
    return "OK " + " ".join(f"{key}={fields[key].replace(' ', ',')}" for key in STATUS_FIELDS if key in fields)


def cmd_selftest(cfg):
    path = os.path.join(cfg["scratch_dir"], "g9toy-selftest.marker")
    try:
        fd = os.open(path, os.O_WRONLY | os.O_CREAT | os.O_EXCL | os.O_NOFOLLOW, 0o600)
    except OSError as exc:
        return err(exc)
    os.close(fd)
    try:
        os.unlink(path)
    except OSError as exc:
        return err(exc) + " (marker created, not removed)"
    return "OK created and removed"


def handle_line(cfg, line):
    """One request line (bytes, newline removed) -> (reply text, log text, keep the connection open)."""
    text = line.decode("utf-8", errors="replace").strip()
    word, _sep, argument = text.partition(" ")
    word = word.upper()
    argument = argument.strip()
    if word == "QUIT":
        return "OK bye", "QUIT", False
    if word == "LIST":
        reply = cmd_list(cfg)
    elif word == "GET":
        reply = cmd_get(cfg, argument)
    elif word == "COUNT":
        reply = cmd_count(cfg)
    elif word == "WHOAMI":
        reply = cmd_whoami()
    elif word == "SELFTEST":
        reply = cmd_selftest(cfg)
    else:
        reply = "ERR 400 unknown command"
    log = f"{printable(word, 12)} {printable(argument, 40)}".strip() + " -> " + printable(reply.split("\n", 1)[0], 60)
    return reply, log, True


class Handler(socketserver.StreamRequestHandler):
    timeout = 5

    def handle(self):
        peer = self.client_address[0]
        for _ in range(MAX_COMMANDS):
            try:
                line = self.rfile.readline(MAX_LINE + 1)
            except OSError:
                return
            if not line:
                return
            if len(line) > MAX_LINE:
                reply, log, keep = "ERR 400 line too long", "line too long", False
            else:
                reply, log, keep = handle_line(self.server.cfg, line.rstrip(b"\r\n"))
            self.wfile.write(reply.encode("utf-8") + b"\n")
            print(f"g9toy peer={peer} {log}", flush=True)
            if not keep:
                return


def main(argv=None):
    parser = argparse.ArgumentParser(description="G9 week 1 toy report service (synthetic)")
    parser.add_argument("--config", required=True, help="path of the configuration file")
    parser.add_argument("--check", action="store_true", help="parse the configuration, print it and exit; opens no socket")
    args = parser.parse_args(argv)
    try:
        with open(args.config, encoding="utf-8") as handle:
            cfg = parse_config(handle.read())
    except (OSError, ValueError) as exc:
        print(f"g9toy: configuration error: {printable(str(exc), 120)}", file=sys.stderr)
        return 2
    if args.check:
        for key in CONFIG_KEYS:
            print(f"{key}={cfg[key]}")
        return 0
    socketserver.TCPServer.allow_reuse_address = True
    try:
        server = socketserver.TCPServer((cfg["bind"], cfg["port"]), Handler)
    except OSError as exc:
        print(f"g9toy: cannot listen on {cfg['bind']}:{cfg['port']}: {err(exc)}", file=sys.stderr)
        return 3
    with server:
        server.cfg = cfg
        print(f"g9toy listening on {cfg['bind']}:{cfg['port']}", flush=True)
        try:
            server.serve_forever()
        except KeyboardInterrupt:
            return 0
    return 0


if __name__ == "__main__":
    sys.exit(main())
