# Security policy

## Reporting a vulnerability

Report vulnerabilities privately through GitHub's private vulnerability
reporting: open the repository's **Security** tab and choose **Report a
vulnerability**. Do not open a public issue. Include the renderer version, the
output format and options, and a scene or asset that reproduces the problem.

## Scope

The renderer is meant to process untrusted scenes within its resource limits.
These are in scope:

- crashes, panics, or aborts on any input;
- CPU time or memory that the resource limits do not bound;
- ways to bypass or exceed a configured resource limit;
- mishandling of malformed or hostile embedded assets (images, nested SVG,
  fonts) that goes beyond the documented recovery.

Rendering differences from Excalidraw are bugs, not vulnerabilities; report
them as ordinary issues. The resource-limit gaps described in
[known limitations](docs/known-limitations.md#resource-limits) are already
known.

## Supported versions

Only the latest 0.x release receives fixes.
