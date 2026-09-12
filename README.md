# 🗿 SigilWard

`Rust` · `sha256`

**File-integrity monitor.** Same category as AIDE/Tripwire — real Rust
rewrites of this are thin on the ground. Baseline a set of paths (hash +
mode + owner + size for every file), then detect drift later: new files,
deleted files, or anything modified — content, permissions, or ownership,
reported separately so you know *what* changed, not just *that* something
did.

A sigil is a mark placed on something to attest it hasn't been tampered
with; that's exactly what this does — seal a file's state, watch for the
seal breaking.

## 🚀 What it does

```bash
sigilward init      # walk every watched path, write a fresh baseline
sigilward check      # compare current state to the baseline, report drift
sigilward update      # same as init — run after reviewing check's output
```

- **`[NEW]`** — a file exists now that wasn't in the baseline
- **`[DELETED]`** — a baselined file is gone
- **`[MODIFIED]`** — content, permissions, and/or ownership changed, reported as separate reasons (a chmod alone shows `(permissions)`, not a false content-change signal)

`check` exits non-zero on any drift — built for cron/systemd-timer use,
not just interactive reading.

## ⚙️ Configuration

`~/.config/sigilward/config.toml`:

```toml
baseline_path = "~/.local/state/sigilward/baseline.json"

[[watch]]
path = "/etc/systemd/system"
recursive = true

[[watch]]
path = "/etc/ssh/sshd_config"

[[watch]]
path = "/home/raven/.ssh/authorized_keys"
```

A path you don't have read access to is skipped with a warning, not a
hard failure — one unreadable file (or one needing root, like
`/etc/sudoers`) shouldn't block auditing everything else. Run via a
root-owned systemd timer instead of interactively if you want full
coverage of root-only files.

## 🧩 Layout

```
src/config.rs     TOML config, XDG resolution
src/baseline.rs   walks watched paths (via walkdir), hashes + stats each
                  file, (de)serializes the baseline
src/diff.rs       compares two baselines -> new/deleted/modified changes
src/format.rs     report rendering, cybercore-themed colors
```

Symlinks are recorded by their own metadata, not followed — a repointed
symlink is itself a change worth catching, and following it risks walking
outside the watched tree entirely.

## 🤖 Automating checks (optional)

A daily systemd timer, the modern equivalent of AIDE's classic cron job:

```ini
# /etc/systemd/system/sigilward-check.service
[Unit]
Description=SigilWard integrity check

[Service]
Type=oneshot
User=raven
ExecStart=/usr/local/bin/sigilward check
```

```ini
# /etc/systemd/system/sigilward-check.timer
[Unit]
Description=Run SigilWard daily

[Timer]
OnCalendar=daily
Persistent=true

[Install]
WantedBy=timers.target
```

## 🗺 Known limitations

- No real-time watching (`inotify`) — this is a point-in-time baseline/compare tool, same operating model as AIDE/Tripwire, not a continuously-running daemon
- A compromised baseline file is a compromised trust anchor — nothing here signs or protects `baseline.json` itself beyond normal filesystem permissions

## 📄 License

MIT
