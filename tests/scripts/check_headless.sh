#!/usr/bin/env bash

chromedriver_path=$(command -v chromedriver)

unameOut="$(uname -s)"
case "${unameOut}" in
    Linux*)     chrome_path="/usr/bin/google-chrome";;
    Darwin*)    chrome_path="/Applications/Google Chrome.app/Contents/MacOS/Google Chrome";;
esac
chromedriver_version=$("${chromedriver_path}" --version)
chrome_version=$("${chrome_path}" --version)

chromedriver_major_version=$("${chromedriver_path}" --version | cut -f 2 -d " " | cut -f 1 -d ".")
chrome_major_version=$("${chrome_path}" --version | cut -f 3 -d " " | cut -f 1 -d ".")

if [ "${chromedriver_major_version}" == "${chrome_major_version}" ]; then
  echo "Chromedriver matches chrome version ✓"
  exit 0
else
  echo "Wasm-Pack often fails with 'invalid session id' if Chromedriver and Chrome have different versions."
  echo "Chromedriver version: ${chromedriver_version} (${chromedriver_path})"
  echo "Chrome version      : ${chrome_version} (${chrome_path})"
  exit 1
fi
