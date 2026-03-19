#!/bin/sh
set -eu

mkdir -p certs

openssl req \
  -x509 \
  -nodes \
  -newkey rsa:2048 \
  -keyout certs/dev-key.pem \
  -out certs/dev-cert.pem \
  -days 365 \
  -subj "/CN=localhost" \
  -addext "subjectAltName=DNS:localhost,IP:127.0.0.1"

printf 'Wrote certs/dev-cert.pem and certs/dev-key.pem\n'
