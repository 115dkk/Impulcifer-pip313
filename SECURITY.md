# Security Policy

Impulcifer-py313 is a personal project maintained by
[@115dkk](https://github.com/115dkk), who is responsible for its security and
handles every vulnerability report for it.

## Scope

- The 3.x app for Windows, macOS and Linux, including its WebView front end,
  the bundled audio service, and the Velopack updater.
- The `impulcifer-py313` package on PyPI: the 3.x CLI and Python API, and the
  2.x Python package.
- The 2.x desktop app and its updater.
- The parsers that read recordings, measurement files and settings.
- The workflows in this repository that build and publish the release files
  and the PyPI package.

Flaws that exist only in Jaakko Pasanen's original Impulcifer belong to that
project. Flaws this fork inherited from it and still carries are in scope.

## Supported versions

- The newest 3.x release on the
  [Releases page](https://github.com/115dkk/Impulcifer-pip313/releases/latest)
  and on PyPI.
- The newest 2.x release. That line is in maintenance and gets security fixes
  on a best-effort basis.

Older versions are not patched. Update first and check whether the problem is
still there.

## Reporting a vulnerability

Report it privately through
[GitHub's private vulnerability reporting](https://github.com/115dkk/Impulcifer-pip313/security/advisories/new).
Do not open a public issue, discussion, or pull request for a suspected
vulnerability.

A useful report names the version, the operating system, how you installed it
(installer, portable build, AppImage, or `pip`), the affected component, and
the steps or the input file that reproduce the problem.

## What happens next

This project is maintained in spare time, so there is no guaranteed response
time. The maintainer reads every report and answers in the report's private
thread. A confirmed vulnerability is fixed in a new release, and the
maintainer then publishes a GitHub security advisory that credits the reporter
unless the reporter asks not to be named.

There is no bug bounty.
