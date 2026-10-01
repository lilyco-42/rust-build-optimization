#!/usr/bin/env python
# -*- coding: utf-8 -*-
"""解析 cargo --timings 表格，并按「proc-macro 链 / 其它」归类"""
import re, sys, os, glob, subprocess, shutil

ROOT = os.path.dirname(os.path.abspath(__file__))

MACRO = ("serde_derive", "syn", "quote", "proc-macro2", "unicode-ident",
         "thiserror-impl", "thiserror")

def secs(s):
    m = re.match(r"([\d.]+)(ms|s|µs)?", s.strip())
    if not m: return 0.0
    v = float(m.group(1)); u = m.group(2) or "s"
    return v * {"s": 1.0, "ms": 0.001, "µs": 1e-6}[u]

def parse(html):
    rows = []
    for tr in re.findall(r"<tr>(.*?)</tr>", html, re.S):
        tds = re.findall(r"<td>(.*?)</td>", tr, re.S)
        if len(tds) < 3: continue
        name = re.sub(r"<.*?>", "", tds[1]).strip()
        dur = secs(re.sub(r"<.*?>", "", tds[2]).strip())
        if dur > 0: rows.append((name, dur))
    return rows

def run(pkg):
    tgt = os.path.join(ROOT, "target", "tm_" + pkg)
    shutil.rmtree(tgt, ignore_errors=True)
    e = dict(os.environ); e.pop("BASH_ENV", None); e.pop("ENV", None)
    e["CARGO_TARGET_DIR"] = tgt
    subprocess.run(["cargo", "build", "-p", pkg, "--timings"], cwd=ROOT, env=e,
                   capture_output=True, text=True, errors="replace")
    fs = sorted(glob.glob(os.path.join(tgt, "cargo-timings", "*.html")))
    if not fs: return None, 0
    html = open(fs[-1], encoding="utf-8").read()
    tot = 0.0
    m = re.search(r"<td>Total time</td>\s*<td>([\d.]+s)</td>", html)
    if not m:
        m = re.search(r"([\d.]+)s</td>\s*</tr>", html)
    return parse(html), secs(re.search(r"Finished.*?in ([\d.]+)s", html).group(1)) if re.search(r"Finished.*?in ([\d.]+)s", html) else 0

for pkg, label in [("lc_dep", "A 现状"), ("lc_nostd", "B no_std+serde-json-core")]:
    rows, wall = run(pkg)
    if not rows: print(pkg, "no data"); continue
    tot = sum(d for _, d in rows)
    macro = sum(d for n, d in rows if any(k in n for k in MACRO))
    print("\n=== %s ===  单元总耗时 %.1fs" % (label, tot))
    for n, d in sorted(rows, key=lambda x: -x[1])[:12]:
        tag = "  ←proc-macro链" if any(k in n for k in MACRO) else ""
        print("   %5.2fs %4.1f%%  %s%s" % (d, d / tot * 100, n, tag))
    print("   proc-macro 链合计 %.2fs = %.1f%%" % (macro, macro / tot * 100))
