# Generate private key
1. `openssl genrsa -out server.key 2048`
2. `openssl ecparam -genkey -name secp384r1 -out server.key`

# Generate self-signed public key
1. `openssl req -new -x509 -sha256 -key server.key -out server.crt -days 3650`

# Read more
https://gist.github.com/denji/12b3a568f092ab951456
