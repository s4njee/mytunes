# Security policy

MyTunes runs a sync server on your local network so that paired phones can download music from the desktop app. Security bugs in that server, in pairing, or in how either app handles files are taken seriously.

## Reporting a vulnerability

**Please don't open a public issue.** Report privately through GitHub's private vulnerability reporting: open the repository's **Security** tab and choose **Report a vulnerability**.

Please include:
- what you found and its impact,
- steps to reproduce (or a proof of concept),
- affected versions or commits, and platforms.

You can expect an acknowledgement within 7 days. Fixes are released as soon as practical, and reporters are credited unless they ask not to be.

## Supported versions

Until 1.0, only the latest release (and `main`) receives security fixes.

## Security model, in brief

- The sync server is off until the user enables Wi-Fi sync or pairs a phone.
- Pairing uses a single-use token that expires after 5 minutes and is rate-limited.
- After pairing, every request uses mutual TLS with pinned certificate fingerprints.
- The server only serves files the library database knows by track ID, never arbitrary paths.

The full design is in `docs/sync-protocol.md` (written in milestone M4).
