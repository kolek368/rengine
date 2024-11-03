protoc --go_out=../backend pong.proto
mkdir -p ../src/pong/protos
protoc --rs_out=../src/pong/protos/ pong.proto

