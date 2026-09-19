`mq_js_bundle.js` is copied from the `js/` directory of the published
Macroquad 0.4.16 crate. It is the browser loader for the pinned Macroquad/Miniquad
dependencies. Keep it in sync when upgrading those dependencies.

Local fix: declare the optional quad-net plugin's `register_plugin` variable with
`var` and terminate that declaration with a semicolon. Upstream assigns it without a declaration under `"use strict"`, which
raises a ReferenceError during script evaluation even when networking is unused.

Upstream: https://github.com/not-fl3/macroquad

Macroquad is dual licensed under MIT or Apache-2.0. The accompanying
`MACROQUAD-LICENSE-MIT` is the upstream MIT license.

The application uses this local copy so running the sandbox requires no CDN.
