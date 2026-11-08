#!/usr/bin/env python3
"""verify_source_register.py - observe what the register's URLs actually serve. Run on a networked host.

This is an ACTIVE network operation, so by default it only prints the plan. Nothing is
requested until you pass --approve-network after reading the plan.

What it does with --approve-network, and nothing more:
  * one HTTPS GET per distinct URL (each entry's `url` and `license_url`), User-Agent
    "g9-source-verifier/0.1", no cookies, no credentials
  * at most 5 redirects, https only; a redirect to a different host is recorded, not hidden
  * reads at most 256 KiB per response; never downloads datasets, archives or binaries
    (a response whose Content-Type is not text/html, text/plain, application/xhtml+xml,
    application/json or application/pdf is recorded and its body is discarded)
  * records HTTP status, final URL, final host, Content-Type, page title and which licence
    keywords occur in the first 256 KiB
  * writes a JSON observation report; it NEVER edits the register

Page text is treated as untrusted data: it is searched for keywords and a title and never executed or followed.
A person must read each licence page and update the register by hand (see
docs/research/reference-source-register.md); scripts/check_source_register.py then enforces the evidence rules.

Usage:
    python3 -I scripts/verify_source_register.py                      # dry run: list hosts and request count
    python3 -I scripts/verify_source_register.py --selftest           # offline checks of the parsing helpers
    python3 -I scripts/verify_source_register.py --approve-network --out verification-report.json [--only SRC-AI-01,SRC-CYBER-03] [--timeout 20]
"""
from __future__ import annotations

import argparse
import hashlib
import json
import re
import sys
import urllib.error
import urllib.request
from pathlib import Path
from urllib.parse import urljoin, urlparse

ROOT = Path(__file__).resolve().parent.parent
REGISTER = ROOT / "data" / "sources" / "reference-source-register.v0.json"
USER_AGENT = "g9-source-verifier/0.1"
MAX_BYTES = 256 * 1024
MAX_REDIRECTS = 5
TEXT_TYPES = ("text/html", "text/plain", "application/xhtml+xml", "application/json", "application/pdf")
KEYWORDS = (
    "creative commons", "cc by", "cc0", "public domain", "apache license", "mit license", "gnu general public",
    "bsd", "detection rule license", "terms of use", "terms of service", "licence", "license", "copyright", "all rights reserved",
)
TITLE = re.compile(r"<title[^>]*>(.*?)</title>", re.IGNORECASE | re.DOTALL)


def extract_title(html_text: str) -> str:
    m = TITLE.search(html_text)
    return " ".join(m.group(1).split())[:200] if m else ""


def license_signals(text: str) -> list[str]:
    low = text.lower()
    return [k for k in KEYWORDS if k in low]


def plan(entries: list[dict], only: set[str] | None) -> list[tuple[str, str, str]]:
    """(entry id, role, url) with each distinct URL requested once, in register order."""
    seen: set[str] = set()
    out = []
    for e in entries:
        if only and e["id"] not in only:
            continue
        for role, key in (("url", "url"), ("license_url", "license_url")):
            u = e[key]
            if u not in seen:
                seen.add(u)
                out.append((e["id"], role, u))
    return out


class _NoRedirect(urllib.request.HTTPRedirectHandler):
    def redirect_request(self, *a, **k):  # we follow redirects by hand to enforce https and the hop limit
        return None


def fetch(url: str, timeout: float) -> dict:
    opener = urllib.request.build_opener(_NoRedirect)
    chain = []
    current = url
    for _ in range(MAX_REDIRECTS + 1):
        req = urllib.request.Request(current, headers={"User-Agent": USER_AGENT, "Accept": "text/html,text/plain,*/*;q=0.5"})
        try:
            resp = opener.open(req, timeout=timeout)
            status = resp.status
            ctype = (resp.headers.get("Content-Type") or "").split(";")[0].strip().lower()
            body = resp.read(MAX_BYTES) if ctype in TEXT_TYPES else b""
            text = body.decode("utf-8", errors="replace")
            return {
                "requested": url, "status": status, "final_url": current, "final_host": urlparse(current).hostname,
                "host_changed": urlparse(current).hostname != urlparse(url).hostname, "redirects": chain,
                "content_type": ctype, "title": extract_title(text) if ctype.startswith(("text/html", "application/xhtml")) else "",
                "license_keywords": license_signals(text), "body_sha256_first_256k": hashlib.sha256(body).hexdigest() if body else "",
                "body_discarded": not body, "error": "",
            }
        except urllib.error.HTTPError as e:
            if e.code in (301, 302, 303, 307, 308) and e.headers.get("Location"):
                nxt = urljoin(current, e.headers["Location"])
                chain.append({"status": e.code, "to": nxt})
                if urlparse(nxt).scheme != "https":
                    return {"requested": url, "status": e.code, "final_url": nxt, "error": "redirect to non-https refused", "redirects": chain}
                current = nxt
                continue
            return {"requested": url, "status": e.code, "final_url": current, "redirects": chain, "error": f"HTTP {e.code}"}
        except Exception as e:  # network errors are observations, not crashes
            return {"requested": url, "status": None, "final_url": current, "redirects": chain, "error": f"{type(e).__name__}: {e}"[:200]}
    return {"requested": url, "status": None, "final_url": current, "redirects": chain, "error": "too many redirects"}


def selftest() -> int:
    assert extract_title("<html><head><TITLE> A \n b </title></head>") == "A b"
    assert extract_title("no title") == ""
    assert "creative commons" in license_signals("Licensed under Creative Commons BY 4.0")
    assert license_signals("nothing relevant here") == []
    entries = [
        {"id": "SRC-X-01", "url": "https://a.example/x", "license_url": "https://a.example/license"},
        {"id": "SRC-X-02", "url": "https://a.example/x", "license_url": "https://b.example/terms"},
    ]
    p = plan(entries, None)
    assert [u for _, _, u in p] == ["https://a.example/x", "https://a.example/license", "https://b.example/terms"], p
    assert plan(entries, {"SRC-X-02"}) == [
        ("SRC-X-02", "url", "https://a.example/x"),
        ("SRC-X-02", "license_url", "https://b.example/terms"),
    ]
    assert urlparse("https://NVD.nist.gov/p").hostname == "nvd.nist.gov"
    print("selftest: OK (no network used)")
    return 0


def main(argv: list[str]) -> int:
    ap = argparse.ArgumentParser(description=__doc__.split("\n")[0])
    ap.add_argument("--approve-network", action="store_true", help="perform the requests listed by the dry run")
    ap.add_argument("--selftest", action="store_true")
    ap.add_argument("--out", default=None)
    ap.add_argument("--only", default=None, help="comma-separated entry ids")
    ap.add_argument("--timeout", type=float, default=20.0)
    args = ap.parse_args(argv[1:])
    if args.selftest:
        return selftest()
    reg = json.loads(REGISTER.read_text(encoding="utf-8"))
    only = set(args.only.split(",")) if args.only else None
    reqs = plan(reg["entries"], only)
    hosts = sorted({urlparse(u).hostname for _, _, u in reqs})
    print(f"plan: {len(reqs)} HTTPS GET request(s) to {len(hosts)} host(s); max {MAX_BYTES // 1024} KiB each; no datasets, no login")
    for h in hosts:
        print("  ", h)
    if not args.approve_network:
        print("dry run only: nothing was requested. Re-run with --approve-network to perform the requests above.")
        return 0
    report = {"tool": USER_AGENT, "register": reg["register_id"], "observations": []}
    for entry_id, role, url in reqs:
        obs = fetch(url, args.timeout)
        obs.update({"entry": entry_id, "role": role})
        report["observations"].append(obs)
        print(f"{entry_id:<14} {role:<11} {obs.get('status')!s:<5} {obs.get('final_host') or ''}  {obs.get('error', '')}")
    text = json.dumps(report, indent=2)
    if args.out:
        Path(args.out).write_text(text + "\n", encoding="utf-8")
        print("wrote", args.out)
    else:
        print(text)
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv))
