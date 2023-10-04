#!/bin/sh
wasm-pack build --release --target web
mv pkg/*.ts lib/web
mv pkg/*.js lib/web
mv pkg/*.wasm lib/web
mv pkg/*.wasm.d.ts lib/web
mv pkg/README.md lib/web
