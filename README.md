## What
Bundle of games created for fun.
***
## Language
RUST, for no particular reason.
***
## Framework
Raylib.
***
## Resources
8-bit audio: https://sfxr.me/
***
## Build
### Dependencies
Multiplayer module uses protobuf to communicate with backend server.
Read more at: https://github.com/stepancheg/rust-protobuf/tree/master/protobuf-codegen
`cargo install protobuf-codegen`
`cd protobuf && ./build_protobuf.sh`

### Main application
`cargo build`

### Backend application
Check readme in **backend** directory 

***
## Run 
`cargo run`
***
## Controls
### Default bindings:  
**Left player**: Q (Up) A (Down)  
**Right player**: P (Up) L (Down)
***
