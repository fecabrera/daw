# Local AUv2 C boundary

This crate independently generates the Apple C interfaces used by our AUv2
exports. It does not use or copy an external AU binding crate. `wrapper.h`,
`symbols.txt`, `build.rs`, `abi.c`, `pin.c` and `cocoa.m` are our source inputs.

See the [architecture contract](../../specs/architecture/auv2-plugin-builds.md)
for generation approval, ownership, ABI rules and source provenance. See the
[developer guide](../../docs/development/plugins.md) for build commands.

Apple system SDK headers carry Apple's copyright and license notices. Obtain the
SDK through Apple's developer tools and follow its accompanying terms. No SDK
headers, generated Rust, preprocessed Apple declarations, SDK binaries or
libclang binaries belong in this crate's source distribution or plugin ZIPs.
`approved.json` contains hashes and tool identities only. Bindgen is a build-only
BSD-3-Clause dependency; its original license is retained in
`assets/licenses/bindgen-BSD-3-Clause.txt`. It is not embedded in the plugin.

The shared Cocoa source compiles separately in each product with checked static
class names. This lets AU hosts discover factories through NSBundle.

Native AU code is enabled only for arm64 macOS. Intel macOS, Windows and Linux
workspace checks compile this crate without generating bindings or linking AU
frameworks. The product crates use the same target guard. Unsupported plugin
package targets are rejected before building.
