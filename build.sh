#!/bin/sh
wasm-pack build --release --target web
mv pkg/*.ts lib/ts
mv pkg/*.js lib/ts
mv pkg/*.wasm lib/ts
mv pkg/*.wasm.d.ts lib/ts
